# 25. `usfm_codegen`: USFM from the AST

Status: resolved
Milestone: M5

The ADR's `usfm_codegen` (AST -> USFM) does not exist. M5's exit is a
property test: parse -> codegen -> parse yields an equal tree on the tcdocs
`pass` corpus.

- New crate `crates/usfm_codegen` depending on `usfm_ast`, `usfm_style`
  (marker names through `document.style_sheet()`), `usfm_span`. Public API:
  `to_usfm_string(&Document) -> String`, implemented as a
  `usfm_ast::visit::Visit` writing into a `String` (the USX writer is the
  model). Output shape: one paragraph per line (`\p`, `\q2` …), `\c N` on its
  own line, `\v N ` inline, `\va`/`\vp` and `\ca`/`\cp` after their number,
  character styles `\nd …\nd*` with `\+` for a nested style (nesting is not
  recorded in the AST: a `Char` inside a `Char` is written with `\+`), notes
  inline `\f + \fr 1:1 \ft …\f*`, attributes `|name="value"` with the default
  attribute written bare when it is the only one, milestones
  `\qt-s |who="…"\*` (no pipe when `attributes` is `None`), tables `\tr \tc1 …
  \tcr2 …`, sidebars `\esb`/`\esbe` with `\cat`, periphs, `\usfm` from
  `Document::usfm_version()`, `//` for `OptBreak`, `~` for U+00A0 in text,
  `\\` for a literal backslash if the parser reads one as an escape (check
  `whitespace.rs`'s escape rules). Verse and chapter end milestones are not
  written (the parser re-emits them).
- Equality ignoring spans: `usfm_ast` gains a `Fold`-based `strip_spans`
  (every span to `SPAN`) or an `eq_ignoring_spans(&Document, &Document)`;
  choose the one that keeps `usfm_ast` small and say why. Two documents also
  differ in their `Arc<StyleSheet>` pointer after a second parse: compare
  marker names, not `StyleId`s, or parse both against the first document's
  sheet.
- Tests: `crates/usfm_codegen/tests/roundtrip.rs` over every tcdocs and
  usfm-grammar `pass` input (walk `tcdocs/tests` and
  `tasks/conformance/fixtures/usfm-grammar/bugfixes` the way `tasks/conformance`
  does; a `pass` case is one whose `metadata.xml` says so): parse, write,
  parse again, assert equal ignoring spans, and assert the second parse's
  diagnostics equal the first's (codes and count). Every failing case is a
  codegen bug to fix here, not a baseline entry; if a case cannot round-trip
  because the parser normalises something irrecoverably (say what), list it
  in a `KNOWN` array in the test with the reason, and ticket it.
- Bench group `codegen` in `tasks/benchmark` (parse outside the loop),
  recorded.

Done when the round-trip test passes on every `pass` input (or the `KNOWN`
list is documented and ticketed) and the gate is green.

## Answer

Landed via PR #30 (2026-09-20). `crates/usfm_codegen` (depends on `usfm_ast`
and `usfm_style` only): `to_usfm_string` / `write_usfm`, a `Visit` writing
bytes straight into a `String`; whole-corpus 233 MiB/s, the cheapest output.
Escapes: `\\`, `\|`, `~` in text, `\\` and `\"` in quoted values, the
default attribute bare and verbatim, `|` flush against a milestone marker
(forced by parser bug 29), `\+` iff the container is a `Char`.
`usfm_ast::eq_ignoring_spans` compares trees by value with markers resolved
through each document's own sheet, since derived `StyleId`s are not stable
across parses. Round trip over 223 conformance `pass` cases and the 86 WEB
books: all pass first time, `KNOWN` empty; the second parse never gains a
code (five lose only `character-style-nested-without-plus`, which is the
writer normalising a spelling), and the output is a fixed point. Facade
feature `codegen`, on by default.

Found on the way, ticketed: 28 (verse end dropped after `\esbe` with no
paragraph marker; the one input in the repo that does not round-trip) and
29 (a block-level milestone with a space before `|` is not a milestone).
