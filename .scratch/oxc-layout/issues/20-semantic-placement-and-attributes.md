# 20. Move placement and stylesheet-driven attribute checks to `usfm_semantic`

Status: resolved
Milestone: M4
Blocked by: 19

`ParserImpl::check_placement` (`marker-not-allowed-here`,
`marker-not-listed-here`: `OccursUnder` from the stylesheet against the parent
marker) and `check_attributes` (`empty-attribute-list`, `no-default-attribute`,
`default-attribute-with-others`, plus `malformed-attribute-name` and
`duplicate-attribute`, which keep the tree as written) need the stylesheet and
the parent, not the token stream.

- `analyze` keeps a marker stack as it walks (paragraph, char, note, table
  cell, sidebar, periph), so the parent name is available where the parser
  had `para_marker`/`open`. Reproduce exactly the parser's rules, including
  the implicit `\p` parent after `\esb`/`\esbe` (ticket 09) and the note-only
  marker outside its note (`\xq` in a paragraph is `marker-not-allowed-here`).
- Ticket 18 (`\ts-s |\*`) lands before this or with it: the empty-list rule
  for milestones is decided there, and moves here as decided.
- Ticket 18 found an asymmetry to resolve here: an *unknown* milestone with
  `|\*` (`\zaln-s |\*`) reports nothing, because `take_unknown_milestone`
  returns an empty `Attributes` both for "no pipe" and "pipe with nothing
  after it" and the parser only checks a non-empty list. In `usfm_semantic`
  the check reads the tree, so it needs to know whether a pipe was present:
  either the AST records it (`Milestone::attributes` as `Option`, an additive
  change, or a `has_pipe` flag) or the parser keeps that one emit. Decide,
  and make `\zaln-s |\*` report `empty-milestone-attribute-list` like
  `\ts-s |\*`.
- The parser deletes the two methods and their state; each moved `Code`'s
  tests and snapshots move to `crates/usfm_semantic/tests/checks.rs`.
- The spans must be identical to today's (the snapshots prove it).

Done when the gate is green with tcdocs unchanged and the parser emits none of
the seven codes.

## Answer

Landed via PR #24 (2026-09-19). `Analyzer` keeps a scope stack (para, char,
note, cell) and reproduces `check_placement` off the tree; sidebars and
periphs need no case since their content sits in its own `Para`. The `\+`
nesting skip falls out of "char inside char: no check", verified by diffing
every input in the repo against a `bb33209` build (one difference: `\fig
||||||` reports `empty-attribute-list` once, not per pipe). Attribute checks
moved with three additive AST changes: `Milestone::attributes` is
`Option<Attributes>` (so `\zaln-s |\*` now reports
`empty-milestone-attribute-list` like `\ts-s |\*`), `Attributes::pipe` and
`Attribute::span`, which keep six of the eight moved codes' spans
byte-identical; the two placement codes report the node span (same start,
wider end). Eight codes in `Code::is_semantic()`; the parser lost
`check_placement`, `check_attributes`, `in_cell`, `in_periph_title` and the
`para_marker` substitution. Three parser snapshots lost one semantic line
each. tcdocs 231 / 0 / 44; corpus zero errors, synthetic Info counts
unchanged.

`parse_semantic` whole-corpus 45.6 vs `parse` 52.1 MiB/s (12%), nearly all
in the attribute re-check; noted on ticket 21 with the `span_check` gap for
the two new span fields.
