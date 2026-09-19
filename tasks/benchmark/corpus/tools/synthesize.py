#!/usr/bin/env python3
"""Derive attribute-heavy and alignment-heavy USFM from the plain WEB corpus.

    python3 synthesize.py --seed 20260919 <web-dir> <out-dir>

The benchmark needs files that exercise the two hot paths a plain scripture
text never reaches: word-level attributes (`\\w word|lemma="..."\\w*`) and
alignment milestones (`\\zaln-s |...\\*` ... `\\zaln-e\\*`). No public project
file with either is available under a licence we can commit, so both are
generated from the World English Bible with markup in the shape
unfoldingWord's aligned texts use. The words, and so the byte volume the
parser sees, are real; the lemmas, Strong's numbers and morphology are not.

Determinism: `--seed` is required, every file gets its own generator seeded
from `seed|class|book` through `random.Random.seed(..., version=2)`, which is
SHA-512 based and therefore stable across processes. Nothing calls `hash()`
on a string, whose value is randomised per process. Running the tool twice
with the same seed gives byte-identical output.
"""

import argparse
import os
import random
import re
import sys

# The books each synthetic class is built from. Attributes are cheap enough
# to carry three: the longest Old Testament prose book, the Psalms (poetry,
# `\q`-dense) and the longest New Testament book. Alignment markup expands
# the source about twenty-sixfold, so that class is Luke alone - the New
# Testament is where unfoldingWord's alignment shape comes from, and one
# book is already 3.8 MB.
BOOKS = {
    "attributes-heavy": ["01-GEN.usfm", "19-PSA.usfm", "43-LUK.usfm"],
    "alignment-heavy": ["43-LUK.usfm"],
}

# Paragraph markers whose content is verse text. Headings (`\ms`, `\d`),
# identification (`\id`, `\h`, `\toc1`) and chapter lines are left alone, as
# a real aligned text leaves them.
VERSE_PARAGRAPHS = {
    "cls", "li", "lim", "m", "mi", "nb", "p", "pc", "ph", "pi", "pm", "pmc",
    "pmo", "pmr", "q", "qm", "qr",
}

# Notes are skipped: unfoldingWord aligns the translation, not its apparatus.
NOTE_MARKERS = {"f", "fe", "ef", "x", "ex"}

MARKER = re.compile(r"\\\+?[A-Za-z][A-Za-z0-9-]*\*?")
# A word is a run of letters or digits, optionally joined by an apostrophe
# (`God’s`). Punctuation and whitespace stay outside the `\w`.
WORD = re.compile(r"[^\W_]+(?:[’'][^\W_]+)*", re.UNICODE)
VERSE_NUMBER = re.compile(r"\s*[0-9]+(?:[-,][0-9]+)*\s*")

MORPHOLOGY = [
    "N,,,,,NMS,", "N,,,,,GMS,", "N,,,,,AMS,", "N,,,,,DFS,", "N,,,,,NMP,",
    "V,IAA3,,S,", "V,IAP3,,P,", "V,PAA,,,NMS,", "EA,,,,NMS,", "RP,,,1,,S,",
    "P,,,,,G,,,", "CC,,,,,,,,,", "AA,,,,NMS,", "D,,,,,,,,,",
]


def language(book_file):
    """Strong's namespace for a book: Hebrew for the Old Testament and the
    deuterocanon, Greek for the New."""
    number = int(book_file[:2])
    return "G" if 41 <= number <= 67 else "H"


def lemma_of(word):
    """A deterministic dictionary-ish form: lower case, possessive dropped."""
    lemma = word.lower()
    for suffix in ("’s", "'s"):
        if lemma.endswith(suffix) and len(lemma) > len(suffix):
            return lemma[: -len(suffix)]
    return lemma


def strong_number(rng, side, width):
    limit = 5624 if side == "G" else 8674
    return f"{side}{rng.randint(1, limit):0{width}d}"


def split_tokens(line):
    """Split a USFM line into ('marker', text) and ('text', text) pieces."""
    pieces = []
    cursor = 0
    for found in MARKER.finditer(line):
        if found.start() > cursor:
            pieces.append(("text", line[cursor:found.start()]))
        pieces.append(("marker", found.group(0)))
        cursor = found.end()
    if cursor < len(line):
        pieces.append(("text", line[cursor:]))
    return pieces


def transform_line(line, wrap):
    """Apply `wrap` to every run of verse text in one paragraph line."""
    pieces = split_tokens(line)
    if not pieces or pieces[0][0] != "marker":
        return line
    base = pieces[0][1][1:].rstrip("0123456789")
    if base not in VERSE_PARAGRAPHS:
        return line
    out = []
    note_depth = 0
    after_verse = False
    for kind, piece in pieces:
        if kind == "marker":
            out.append(piece)
            name = piece[1:].rstrip("*")
            if name in NOTE_MARKERS:
                note_depth += 1 if not piece.endswith("*") else -1
            after_verse = name == "v" and not piece.endswith("*")
            continue
        text = piece
        if after_verse:
            # `\v 12 ` - the number is not a word of the verse.
            number = VERSE_NUMBER.match(text)
            if number:
                out.append(number.group(0))
                text = text[number.end():]
            after_verse = False
        out.append(text if note_depth > 0 else wrap(text))
    return "".join(out)


def attributes_wrapper(rng, side):
    def wrap(text):
        def replace(found):
            word = found.group(0)
            return (
                f'\\w {word}|lemma="{lemma_of(word)}" '
                f'strong="{strong_number(rng, side, 4)}"\\w*'
            )

        return WORD.sub(replace, text)

    return wrap


def alignment_wrapper(rng, side):
    """Wrap word groups in `\\zaln-s |...\\*` ... `\\zaln-e\\*`.

    Roughly one group in six spans two words, the way a real alignment binds
    two English words to one original-language word.
    """
    prefix = "Gr," if side == "G" else "He,"

    def aligned_word(word):
        return f'\\w {word}|x-occurrence="1" x-occurrences="1"\\w*'

    def wrap(text):
        words = list(WORD.finditer(text))
        if not words:
            return text
        out = []
        cursor = 0
        index = 0
        while index < len(words):
            span = 2 if (index + 1 < len(words) and rng.randrange(6) == 0) else 1
            group = words[index:index + span]
            out.append(text[cursor:group[0].start()])
            surface = [found.group(0) for found in group]
            content = " ".join(word.lower() for word in surface)
            out.append(
                f'\\zaln-s |x-strong="{strong_number(rng, side, 5)}" '
                f'x-lemma="{lemma_of(surface[0])}" '
                f'x-morph="{prefix}{rng.choice(MORPHOLOGY)}" '
                f'x-occurrence="1" x-occurrences="1" x-content="{content}"\\*'
            )
            for position, found in enumerate(group):
                if position:
                    out.append(text[group[position - 1].end():found.start()])
                out.append(aligned_word(found.group(0)))
            out.append("\\zaln-e\\*")
            cursor = group[-1].end()
            index += span
        out.append(text[cursor:])
        return "".join(out)

    return wrap


CLASSES = {
    "attributes-heavy": attributes_wrapper,
    "alignment-heavy": alignment_wrapper,
}


def main(argv):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--seed", required=True, help="generator seed; the committed files use 20260919")
    parser.add_argument("web_dir", help="directory of plain WEB USFM files")
    parser.add_argument("out_dir", help="directory the synthetic classes are written to")
    options = parser.parse_args(argv[1:])

    for class_name in sorted(CLASSES):
        target = os.path.join(options.out_dir, class_name)
        os.makedirs(target, exist_ok=True)
        for book_file in BOOKS[class_name]:
            rng = random.Random()
            rng.seed(f"{options.seed}|{class_name}|{book_file}", version=2)
            wrap = CLASSES[class_name](rng, language(book_file))
            with open(os.path.join(options.web_dir, book_file), encoding="utf-8") as handle:
                source = handle.read()
            lines = [transform_line(line, wrap) for line in source.split("\n")]
            with open(os.path.join(target, book_file), "w", encoding="utf-8", newline="\n") as handle:
                handle.write("\n".join(lines))
            sys.stderr.write(f"{class_name}/{book_file}\n")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
