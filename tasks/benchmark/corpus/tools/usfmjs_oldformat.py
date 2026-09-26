#!/usr/bin/env python3
"""Close the open milestones of usfm-js's "oldformat" aligned USFM.

unfoldingWord's aligned texts from before USFM 3.0 was final write a
milestone's attribute list to the end of its line and never close it:

    \\zaln-s |x-strong="G35880" x-lemma="ὁ" ... x-content="τὸν"
    \\w The|x-occurrence="1" x-occurrences="1"\\w*
    \\zaln-e\\*

USFM 3 requires the `\\*` (`\\zaln-s |...\\*`), which is how usfm-js itself has
written them since, and how tcdocs' `usfmjsTests` judges them: the
`*.oldformat` cases are `validated=fail`. This script makes that one edit and
no other: every `\\zaln-s |` or `\\k-s |` whose attribute list runs to the end
of its line gets `\\*` appended to that line. Nothing is removed, reordered or
re-spaced, so the output is the input with `\\*` inserted, which `main`
checks before it writes anything.

Python 3 standard library only, no network, deterministic.

    python3 tools/usfmjs_oldformat.py <in.usfm> <out.usfm>
"""

import re
import sys

# A start milestone whose attribute list reaches the end of the line: no `\`
# (so no `\*`, and no other marker) between the `|` and the line break.
OPEN_MILESTONE = re.compile(r"(\\(?:zaln|k)-s \|[^\\\n]*)$", re.MULTILINE)


def close_milestones(text):
    """Return `text` with `\\*` after every unclosed milestone, and the count."""
    return OPEN_MILESTONE.subn(r"\1\\*", text)


def main(argv):
    if len(argv) != 3:
        sys.stderr.write(__doc__.splitlines()[-1].strip() + "\n")
        return 2
    with open(argv[1], encoding="utf-8", newline="") as source:
        text = source.read()
    closed, count = close_milestones(text)
    # The only change is the inserted `\*`: deleting every one of them from
    # both sides must give the same text.
    if closed.replace("\\*", "") != text.replace("\\*", "") or len(closed) != len(text) + 2 * count:
        raise SystemExit(f"{argv[1]}: the edit changed more than the milestone closers")
    with open(argv[2], "w", encoding="utf-8", newline="") as out:
        out.write(closed)
    sys.stderr.write(f"{argv[2]}: {count} milestones closed\n")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
