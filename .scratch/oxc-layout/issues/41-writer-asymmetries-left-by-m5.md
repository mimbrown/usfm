# 41. Two writer/parser asymmetries M5 left on purpose

Status: resolved
Milestone: after M6

Both are recorded in code comments and round-trip correctly; neither is a
bug. They are listed so they are not rediscovered.

1. **`\cp`'s number is one raw `Word` token kept as text.** `parse_chapter`
   reads the published chapter number as a single lexer word and
   `usfm_codegen` writes it verbatim, where `\vp`'s number is a text run
   written through the escaper (ticket 35). So `\cp` cannot carry a `\`,
   a `|` or a `"` at all, and a `~` in it is a `~`, not a no-break space.
   Making `\cp` read text would move spans and change `pub_number`'s
   meaning for every consumer (USX writes it as an attribute); do it only
   if a real project needs a published number with such a character.
2. **The writer keeps an `\xo*` before a closed `\xt`.** Since ticket 36
   nothing nests inside `\xo` without `\+`, so `\xo 1.1 \xt b|link-href="x"\xt*`
   would read back identically with the `\xo*` left out; `omitted_closers`'
   `!nest || omitted[i+1]` clause is one case wider than the parser needs.
   No corpus writes the shape. Narrowing the clause to "the style being
   nested into is not `\xo`" mirrors `parent_holds_plain_text` and is a
   five-line change with `a_closed_xt_after_xo_is_written_as_a_sibling`
   as the test to update.

Done when either is changed with its test, or this ticket is marked
`wontfix` with the reason.

## Answer

Picked by Michael on 2026-09-26.

1. **`\cp`: wontfix.** No project in either conformance root, the benchmark
   corpus or the machine.py fixtures writes a published chapter number
   with a `\`, `|`, `"` or `~`, and reading it as text would move spans and
   change `pub_number` for every consumer (USX writes it as an attribute).
   Reopen as a new ticket if a real project needs it.
2. **`\xo*`: changed.** `omitted_closers` now drops the closer of an `\xo`
   before any note-internal style, mirroring the parser's
   `parent_holds_plain_text`, so `\xo 1.1 \xt b|link-href="x"\xt*` is
   written without the `\xo*`. `a_closed_xt_after_xo_is_written_as_a_sibling`
   asserts the new spelling and that it reads back to the same tree; the
   round-trip suites are unchanged.
