# 20. Move placement and stylesheet-driven attribute checks to `usfm_semantic`

Status: ready-for-agent
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
- The parser deletes the two methods and their state; each moved `Code`'s
  tests and snapshots move to `crates/usfm_semantic/tests/checks.rs`.
- The spans must be identical to today's (the snapshots prove it).

Done when the gate is green with tcdocs unchanged and the parser emits none of
the seven codes.
