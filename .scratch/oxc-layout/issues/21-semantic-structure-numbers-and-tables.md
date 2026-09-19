# 21. Audit every `Code`: what stays in the parser, what moves

Status: ready-for-agent
Milestone: M4
Blocked by: 20

Go through `Code::ALL` with its doc rows. A code whose **Recovery** row is
"none" or "kept as written" is a document check and moves to `usfm_semantic`;
a code whose recovery changes the tree stays in the parser (it is how the
tree got its shape). Candidates to move, to be confirmed against the rows:
`number-has-leading-zero`, `unexpected-table-column`,
`verse-text-before-chapter`, `verse-in-heading`, `verse-in-note`,
`verse-in-character-style`, `verse-outside-chapter`, `missing-id`,
`empty-book`, `id-not-first`, `unknown-custom-milestone` (kept, Info),
`character-style-nested-without-plus` (Info, but it *is* the nesting
decision: probably stays). `unknown-marker` and everything that drops or
re-parents content stays.

- Write the table (code, severity, recovery, stays/moves, why) into this
  ticket's answer and into `crates/usfm_diagnostics/src/diagnostics.rs`'s
  module doc as the new rule: "a parser code repairs; a semantic code
  reports".
- Spans: a moved check reports the node's span, which can be wider than the
  parser's token span (ticket 19: `unlisted-book-code` went from the code
  word `@4..7` to the whole `\id` line). Decide once for all moved codes
  whether a narrower span matters for the language server; if it does, give
  `Book` (and any other node whose check needs a sub-span) an additive
  field for it, filled by the parser.
- Move what the table says, tests and snapshots with them, as in ticket 20.
- `ParserImpl::check_document_structure` goes if everything in it moved.

Done when the gate is green with tcdocs unchanged and the module doc's table
matches the emitters (a unit test in `usfm_semantic` that every code it emits
is in its list, and one in the parser likewise, keeps the table honest).
