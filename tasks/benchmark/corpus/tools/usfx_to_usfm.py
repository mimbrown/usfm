#!/usr/bin/env python3
"""Convert an eBible.org USFX file into one USFM 3 file per book.

    python3 usfx_to_usfm.py <eng-web.usfx.xml> <out-dir>

USFX is eBible.org's own lossless XML rendering of the USFM it publishes, so
the conversion is a mechanical un-tagging: `<p sfm="ip">` is `\\ip`, `<q
level="2">` is `\\q2`, `<v id="1"/>` is `\\v 1 `, `<f caller="+">` is `\\f +
... \\f*`, and so on. The output is meant to look like the USFM eBible.org
distributes: one line per paragraph, verses inline, notes inline.

Only the standard library is used and nothing is fetched at run time; the
input file is an argument. The output is byte-for-byte deterministic: the
books are visited in document order and nothing is hashed or sorted.

Any element the converter does not know raises `Unsupported`, so the list of
handled elements in ../README.md is exact and a new USFX revision cannot be
silently dropped on the floor.
"""

import os
import re
import sys
import xml.etree.ElementTree as ET

# USFM book-identifier numbers (USFM 3.0, "Book Identifiers"): 01-39 Old
# Testament, 41-67 New Testament, 68-85 deuterocanon. The two peripheral
# books the USFX carries are numbered here, outside that table, so that every
# file has a numeric prefix: front matter sorts first and the glossary last.
BOOK_NUMBER = {
    "FRT": "00",
    "GEN": "01", "EXO": "02", "LEV": "03", "NUM": "04", "DEU": "05",
    "JOS": "06", "JDG": "07", "RUT": "08", "1SA": "09", "2SA": "10",
    "1KI": "11", "2KI": "12", "1CH": "13", "2CH": "14", "EZR": "15",
    "NEH": "16", "EST": "17", "JOB": "18", "PSA": "19", "PRO": "20",
    "ECC": "21", "SNG": "22", "ISA": "23", "JER": "24", "LAM": "25",
    "EZK": "26", "DAN": "27", "HOS": "28", "JOL": "29", "AMO": "30",
    "OBA": "31", "JON": "32", "MIC": "33", "NAM": "34", "HAB": "35",
    "ZEP": "36", "HAG": "37", "ZEC": "38", "MAL": "39",
    "MAT": "41", "MRK": "42", "LUK": "43", "JHN": "44", "ACT": "45",
    "ROM": "46", "1CO": "47", "2CO": "48", "GAL": "49", "EPH": "50",
    "PHP": "51", "COL": "52", "1TH": "53", "2TH": "54", "1TI": "55",
    "2TI": "56", "TIT": "57", "PHM": "58", "HEB": "59", "JAS": "60",
    "1PE": "61", "2PE": "62", "1JN": "63", "2JN": "64", "3JN": "65",
    "JUD": "66", "REV": "67",
    "TOB": "68", "JDT": "69", "ESG": "70", "WIS": "71", "SIR": "72",
    "BAR": "73", "LJE": "74", "S3Y": "75", "SUS": "76", "BEL": "77",
    "1MA": "78", "2MA": "79", "3MA": "80", "4MA": "81", "1ES": "82",
    "2ES": "83", "MAN": "84", "PS2": "85",
    "GLO": "88",
}

# `<p sfm="...">` values that map to a paragraph marker of the same name. A
# `<p>` with no `sfm` is `\p`; `level="2"` appends the level (`\ili2`).
PARAGRAPH_SFM = {
    "ili", "ip", "is", "li", "m", "mi", "ms", "mt", "mte", "nb", "p", "pc",
    "pi", "sp",
}

# Character styles inside a paragraph, written `\tag text\tag*`.
CHAR_TAGS = {"add", "bk", "it", "k", "qs", "vp", "wj"}

# Runs inside a note, written `\tag text` and closed implicitly by the next
# run or by `\f*` / `\x*`, the way eBible.org's USFM writes them.
NOTE_RUNS = {"fr", "ft", "fq", "fqa", "fl", "xo", "xt"}

# `<ref tgt="...">` has no USFM marker of its own: it is USFX bookkeeping
# around text that is already inside an `\xt`/`\ft` run. Drop the tag, keep
# the text.
TRANSPARENT_TAGS = {"ref"}

WHITESPACE = re.compile(r"[ \t\r\n]+")


class Unsupported(Exception):
    """A USFX element or attribute the converter has no rule for."""


class Line:
    """One USFM line, assembled marker by marker.

    Source whitespace is preserved (collapsed to single spaces) rather than
    invented: eBible.org writes `God\\f + ...\\f* created`, with no space in
    front of the note, and the converter must not add one. Only a closing
    marker trims, so a note ends `...text\\f*` and not `...text \\f*`.
    """

    def __init__(self):
        self.parts = []

    def text(self, raw):
        if not raw:
            return
        chunk = WHITESPACE.sub(" ", raw)
        if not self.parts or self.parts[-1].endswith(" "):
            chunk = chunk.lstrip(" ")
        if chunk:
            self.parts.append(chunk)

    def open(self, marker):
        """`\\marker ` - no space is added in front of it."""
        self.parts.append("\\" + marker + " ")

    def close(self, marker):
        self.rstrip()
        self.parts.append("\\" + marker + "*")

    def standalone(self, marker, argument=None):
        """`\\marker arg `, separated from what precedes it by one space."""
        self.rstrip()
        if self.parts:
            self.parts.append(" ")
        if argument is None:
            self.parts.append("\\" + marker)
        else:
            self.parts.append("\\" + marker + " " + argument + " ")

    def rstrip(self):
        while self.parts:
            self.parts[-1] = self.parts[-1].rstrip(" ")
            if self.parts[-1]:
                break
            self.parts.pop()

    def render(self):
        self.rstrip()
        return "".join(self.parts)


def flat_text(line, element, put):
    """Walk mixed content, sending every text run to `put`."""
    put(element.text)
    for child in element:
        if child.tag in TRANSPARENT_TAGS:
            flat_text(line, child, put)
        else:
            raise Unsupported(f"<{child.tag}> inside <{element.tag}>")
        put(child.tail)


def emit_note(line, element, tag, default_run):
    """`<f caller="+">` -> `\\f + \\fr 1:1 \\ft note\\f*`.

    A note is a sequence of runs. Text that is not inside a run element -
    which is how the USFX carries most WEB footnotes - opens the note's
    default run (`\\ft` for `\\f`, `\\xt` for `\\x`), and so does text that
    follows a run.
    """
    caller = element.get("caller")
    if caller is None:
        raise Unsupported(f"<{tag}> without a caller")
    caller = caller.strip()
    leading = element.text or ""
    if not caller:
        # USFX quirk: one WEB footnote (2 Esdras 1:5) carries its caller as
        # the element's leading text rather than in the attribute.
        rest = leading.lstrip()
        if rest[:1] in ("+", "-"):
            caller, leading = rest[0], rest[1:]
        else:
            raise Unsupported(f"<{tag}> with an empty caller")
    # `open`, not `standalone`: a note sits directly against the word it
    # annotates, as `God\f + ...\f* created`.
    line.open(tag + " " + caller)
    state = {"run": None}

    def put(raw):
        if not raw:
            return
        if raw.strip():
            if state["run"] is None:
                line.open(default_run)
                state["run"] = default_run
            line.text(raw)
        elif state["run"] is not None:
            line.text(raw)

    put(leading)
    for child in element:
        if child.tag in TRANSPARENT_TAGS:
            flat_text(line, child, put)
        elif child.tag in NOTE_RUNS:
            line.open(child.tag)
            state["run"] = child.tag
            flat_text(line, child, line.text)
        else:
            raise Unsupported(f"<{child.tag}> inside <{tag}>")
        # Text after a run starts a fresh default run.
        state["run"] = None if child.tag in NOTE_RUNS else state["run"]
        put(child.tail)
    line.close(tag)


def emit_inline(line, element):
    """Mixed content of a paragraph, a `\\d`, or a character style."""
    line.text(element.text)
    for child in element:
        tag = child.tag
        if tag == "v":
            number = child.get("id")
            if number is None:
                raise Unsupported("<v> without an id")
            line.standalone("v", number)
        elif tag == "ve":
            # Verse ends are implicit in USFM; the parser emits them itself.
            pass
        elif tag == "f":
            emit_note(line, child, "f", "ft")
        elif tag == "x":
            emit_note(line, child, "x", "xt")
        elif tag in CHAR_TAGS:
            line.open(tag)
            emit_inline(line, child)
            line.close(tag)
        elif tag in TRANSPARENT_TAGS:
            emit_inline(line, child)
        else:
            raise Unsupported(f"<{tag}> inside <{element.tag}>")
        line.text(child.tail)


def paragraph_marker(element):
    if element.tag == "q":
        base = "q"
    else:
        base = element.get("sfm") or "p"
        if base not in PARAGRAPH_SFM:
            raise Unsupported(f'<p sfm="{base}">')
    level = element.get("level")
    if level is None:
        return base
    if not level.isdigit():
        raise Unsupported(f'<{element.tag} level="{level}">')
    return base + level


def convert_book(book):
    """Return the USFM text of one `<book>`, ending in a newline."""
    book_id = book.get("id")
    if book_id is None:
        raise Unsupported("<book> without an id")
    lines = []
    for element in book:
        tag = element.tag
        line = Line()
        if tag == "id":
            line.standalone("id", element.get("id") or book_id)
            line.text(element.text)
        elif tag == "ide":
            charset = element.get("charset")
            if charset is None:
                raise Unsupported("<ide> without a charset")
            line.standalone("ide", charset)
        elif tag == "h":
            line.standalone("h", None)
            line.text(" ")
            line.text(element.text)
        elif tag == "toc":
            level = element.get("level")
            if level not in {"1", "2", "3", "4"}:
                raise Unsupported(f'<toc level="{level}">')
            if level == "4":
                # USFM 3 defines \toc1-\toc3 only. The USFX carries a fourth
                # level for Psalms holding the same text as the <cl> element
                # two lines further down, so nothing is lost by leaving it
                # out - and \toc4 is not a marker the stylesheet knows.
                continue
            line.standalone("toc" + level, None)
            line.text(" ")
            line.text(element.text)
        elif tag == "c":
            number = element.get("id")
            if number is None:
                raise Unsupported("<c> without an id")
            line.standalone("c", number)
        elif tag == "cp":
            number = element.get("id")
            if number is None:
                raise Unsupported("<cp> without an id")
            line.standalone("cp", number)
        elif tag == "cl":
            line.standalone("cl", None)
            line.text(" ")
            line.text(element.text)
        elif tag == "b":
            line.standalone("b", None)
        elif tag == "d":
            line.standalone("d", None)
            line.text(" ")
            emit_inline(line, element)
        elif tag in ("p", "q"):
            line.standalone(paragraph_marker(element), None)
            line.text(" ")
            emit_inline(line, element)
        else:
            raise Unsupported(f"<{tag}> inside <book>")
        lines.append(line.render())
    return "".join(text + "\n" for text in lines)


def file_name(book_id):
    number = BOOK_NUMBER.get(book_id)
    if number is None:
        raise Unsupported(f"no book number for {book_id}")
    return f"{number}-{book_id}.usfm"


def main(argv):
    if len(argv) != 3:
        sys.stderr.write(f"usage: {argv[0]} <usfx.xml> <out-dir>\n")
        return 2
    source, out_dir = argv[1], argv[2]
    root = ET.parse(source).getroot()
    os.makedirs(out_dir, exist_ok=True)
    written = 0
    for element in root:
        if element.tag == "languageCode":
            continue
        if element.tag != "book":
            raise Unsupported(f"<{element.tag}> inside <usfx>")
        name = file_name(element.get("id"))
        text = convert_book(element)
        with open(os.path.join(out_dir, name), "w", encoding="utf-8", newline="\n") as handle:
            handle.write(text)
        written += 1
    sys.stderr.write(f"{written} books written to {out_dir}\n")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
