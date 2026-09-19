# 21. Audit every `Code`: what stays in the parser, what moves

Status: resolved
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
- Ticket 20 added `Attributes::pipe` and `Attribute::span`; extend
  `usfm_parser::span_check` to assert them (in bounds, on a boundary, the
  pipe is one byte of `|`, the name span starts the name) so the fuzz targets
  cover them.
- `parse_semantic` is 12% under `parse` after ticket 20, nearly all of it the
  attribute re-check (`attributes-heavy` 15% apart). After this ticket's
  moves, measure again and, if the gap is over 10%, ticket a targeted fix
  (walk attribute lists once, or check them on the way in the parser's
  `Attributes` builder and only *report* from the pass).

Done when the gate is green with tcdocs unchanged and the module doc's table
matches the emitters (a unit test in `usfm_semantic` that every code it emits
is in its list, and one in the parser likewise, keeps the table honest).

## Comments

### The audit

Every variant of `Code::ALL`, against the two questions in the rule. A code
stays with the parser if **deleting the check would change the tree**, or if
**what the author wrote is no longer visible in the tree** — a dropped token, a
normalised number, a `+` or a `\ca*` that is not recorded, or a repair standing
in for the input. Everything else moves. The same table is now the
`usfm_diagnostics` module doc (`Code::is_semantic` is its executable form, and
`usfm_semantic::EMITS` this crate's half; a test asserts the two agree).

| Code | Sev | What the parser does with it | Verdict | Why |
|---|---|---|---|---|
| `unknown-marker` | E | drops the marker | parser | the marker is gone |
| `unknown-custom-marker` | W | drops the marker | parser | the marker is gone |
| `unknown-milestone` | W | registers the style, keeps the node | parser | the tree does not say which styles the sheet lacked, and it is reported once per name, not per use |
| `unknown-custom-milestone` | I | registers the style, keeps the node | parser | as above (the ticket guessed it would move) |
| `unmatched-closing-marker` | E | drops the marker | parser | the marker is gone |
| `paragraph-marker-closed` | E | drops the marker | parser | the marker is gone |
| `unmatched-milestone-end` | E | drops the `\*` | parser | the token is gone |
| `milestone-not-closed` | E | closes the milestone early | parser | it decides where the node ends |
| `stray-backslash` | E | keeps the `\` as text | parser | in the tree it is a backslash like any other, escaped or not |
| `marker-not-allowed-here` | E | nothing | semantic (20) | the style, its parent and the sheet are in the tree |
| `marker-not-listed-here` | I | nothing | semantic (20) | as above |
| `nested-marker-not-nested` | W | reads `\+x` as `\x` | parser | whether the `+` was written is not in the tree |
| `character-style-not-closed` | W | closes the style there | parser | it decides where the node ends |
| `character-style-implicitly-closed` | I | closes one style, opens a sibling | parser | it is the sibling-or-child decision |
| `character-style-nested-without-plus` | I | nests the style | parser | it is the nesting decision, and the `+` is not in the tree |
| `figure-not-closed` | E | closes the figure there | parser | it decides where the node ends |
| **`empty-word`** | E | nothing | **moved (21)** | a `\w` with attributes and no children is the whole rule |
| `note-not-closed` | E | closes the note there | parser | it decides where the node ends |
| `missing-note-caller` | E | assumes `+` | parser | it invents the caller |
| `missing-verse-number` | E | drops `\v` | parser | the marker is gone |
| `malformed-verse-number` | E | drops `\v` and the number | parser | the marker is gone |
| `verse-in-note` | E | drops `\v` and the number | **parser** | the verse is not in the tree to report on (the ticket listed it as a candidate) |
| **`verse-in-character-style`** | W | nothing | **moved (21)** | a `VerseStart` inside a `Char` |
| **`verse-in-heading`** | E | nothing | **moved (21)** | a `VerseStart` in a `Title`/`Section` paragraph |
| **`verse-outside-chapter`** | E | nothing | **moved (21)** | a `VerseStart` before any `ChapterStart` |
| **`verse-text-before-chapter`** | E | nothing | **moved (21)** | a verse-text `Para` before any `ChapterStart` |
| `missing-chapter-number` | E | drops `\c` | parser | the marker is gone |
| `malformed-chapter-number` | E | drops `\c` and the number | parser | the marker is gone |
| `number-has-leading-zero` | E | reads the number without the zero | **parser** | `01` is the number 1 in the tree; `NumberList`/`ChapterStart::number` keep a `usize`, so nothing remembers the zero |
| `alternate-chapter-not-closed` | E | keeps the number, carries on | parser | a closed and an unclosed `\ca` give the same `alt_number` |
| `alternate-verse-not-closed` | E | keeps the number, carries on | parser | as above |
| `missing-book-code` | E | drops the `\id` line | parser | there is no `Book` in the tree |
| `unknown-book-code` | E | drops the `\id` line | parser | there is no `Book` in the tree |
| `unlisted-book-code` | W | nothing | semantic (19) | the `Book` is kept as written |
| **`missing-id`** | E | nothing | **moved (21)** | the document's first block is not a `Book` |
| **`id-not-first`** | E | nothing | **moved (21)** | a `Book` that is not the first block of its list (the parser asked the same of the list it was appending to) |
| **`empty-book`** | E | nothing | **moved (21)** | the document's only block is a `Book` |
| `sidebar-not-closed` | E | closes the sidebar at `\c`, the next `\esb` or EOF | parser | a closed and an unclosed sidebar are the same node (the doc row said "Recovery: none"; it is fixed) |
| `unmatched-sidebar-end` | E | keeps the `\esbe` as an empty paragraph | parser | that paragraph is the repair, not anything the author wrote (doc row fixed likewise) |
| `content-outside-paragraph` | E | opens an implicit `\p` | parser | it invents the node |
| `content-dropped` | E | drops the content | parser | the content is gone |
| `expected-table-cell` | E | opens an implicit `\tc1` | parser | it invents the node |
| **`unexpected-table-column`** | E | keeps the column the marker named | **moved (21)** | `TableCell::column` in row order |
| `unexpected-pipe` | E | keeps the `|` as text | parser | an escaped `\|` reaches the tree as the same character |
| `unterminated-attribute-value` | E | ends the value at the marker | parser | it decides what the value is |
| `newline-in-attributes` | E | reads the break as a space | parser | the break is not in the tree |
| `empty-attribute-list` | E | keeps an empty list | semantic (20) | `Attributes` with no pairs, and its `|` |
| `empty-milestone-attribute-list` | W | keeps an empty list | semantic (20) | as above |
| `no-default-attribute` | E | keeps the bare value | semantic (20) | a pair with an empty name |
| `default-attribute-with-others` | E | keeps the bare value | semantic (20) | as above |
| `attribute-value-not-quoted` | E | takes the one word after `=` | parser | it decides what the value is; the quotes are not in the tree |
| `missing-attribute-value` | E | keeps an empty value | parser | `name=` and `name=""` are the same pair in the tree |
| `malformed-attribute-name` | E | keeps the name as written | semantic (20) | the pair is in the tree, name and span |
| `duplicate-attribute` | E | keeps every occurrence | semantic (20) | as above |
| `internal` | E | stops parsing | parser | it is the parser's own invariant |

Nine moved (eight the ticket listed, minus `verse-in-note` and
`number-has-leading-zero`, plus `empty-word`, which the audit turned up and
which is the same predicate on the same node). 18 codes are semantic now.

### The span decision

**No sub-span fields, and no `Book.code_span`: M6 revisits.** Every moved check
reports the node it read, which starts at the offset the parser reported and
ends later; a diagnostic on the whole `\id` line, the whole `\v 1` or the whole
cell is as usable in an editor as one on the marker, and no language server
exists to be fussy until M6. The one case where a node span would have been
misleading — a whole-table span for one cell — does not arise, because
`unexpected-table-column` reports the `TableCell`, not the `Table`.
27 diagnostics over the 385 inputs in the repo have a wider end; nothing else
about them changed.

`Document` did gain a `span` (`0..len`, `SPAN` for a hand-built tree, additive:
`Document::new` is unchanged and `with_span` sets it). `empty-book` reports at
the end of the source, and that offset was in no node; the alternative was to
move the report to the end of the `\id` line, which would have been the first
span in either move to start somewhere new.

### `span_check`, the empirical diff and the bench

`usfm_parser::span_check` now walks the attribute list of every `Char`,
`Milestone` (inline and between blocks) and `Periph`: the `|` must be exactly
one byte of `|` (a new `Prefix::Exact`, or `SPAN` for a hand-built list) and
each pair's span must be in bounds, on character boundaries and start with the
name it was read from — or, for a bare value, with the value run, which the
parser borrows verbatim. `spans.rs` gained
`attribute_lists_on_every_node_that_can_carry_one`, four inputs covering all
three nodes, named, bare, quoted and unquoted values, and the four recovered
shapes (no attribute, a malformed name, an unterminated value, a line break in
the list). Breaking either assertion on purpose fails 5 of the 15 tests, so
they are live. `cargo +nightly fuzz run --fuzz-dir tasks/fuzz parse_utf8 --
-max_total_time=120 -max_len=65536`: 15 501 runs, no crash, no assertion.

Empirical diff against a `dea9548` build of the CLI, `usfm parse <file>
--diagnostics json` over all 385 `.usfm`/`.SFM`/`.txt` inputs under
`tcdocs/tests`, `tasks/conformance/fixtures` and `tasks/benchmark/corpus`:
24 938 diagnostics on each side, **none gained and none lost**. The only
differences are 27 wider span ends (9 `verse-text-before-chapter`, 9
`verse-outside-chapter`, 5 `verse-in-heading`, 2 `unexpected-table-column`, 1
`verse-in-character-style`, 1 `id-not-first`) and the two
`unexpected-table-column` messages, which now name the column. `--format usx`
over the same 385 files is byte-identical. tcdocs: 231 / 0 / 44, baseline
still empty.

Two rules changed where the tree is the better witness, neither reachable by
anything in the repo (hence no diff): a verse in a **table cell** is no longer
"in a heading" because a heading paragraph came before the table (the parser
read the text type of the last paragraph marker it had passed; the tree says
the verse is in a cell), and a verse-text paragraph the parser *made* out of
content after `\esb`/`\esbe` is now checked as the `\p` it is in the tree
rather than as the `\esb` line it was read from. Both have a test in
`checks.rs`.

`parse_semantic` whole-corpus **45.89** MiB/s against `parse` at **51.51**,
three rounds turn about: a **10.9%** gap, against 12.4% after ticket 20, so
the nine checks moved here cost nothing above the noise. The remaining gap is
still the attribute re-check — `plain` 1.8%, `note-heavy` 7.1%,
`alignment-heavy` 14.4%, `attributes-heavy` 17.9%, the two heavy classes being
the two full of attribute lists. Recorded in `docs/benchmarks.md`; still over
the ~10% this ticket set as the threshold for a targeted fix, and the cause is
unchanged from ticket 20's finding.

## Answer

Landed via PR #25 (2026-09-19). All 55 codes audited (table in the
diagnostics module doc and under Comments above): 18 are semantic now. Moved
here: `missing-id`, `id-not-first`, `empty-book`, `verse-text-before-chapter`,
`verse-outside-chapter`, `verse-in-heading`, `verse-in-character-style`,
`unexpected-table-column`, and `empty-word` (the predicate already read the
finished node). Stayed with reasons: `verse-in-note` (the verse is dropped),
`number-has-leading-zero` (the zero is normalised away),
`unknown-(custom-)milestone` (per name, not per use), the nesting decision.
Span decision: no sub-span fields; M6 revisits; one additive `Document::span`
so `empty-book` can point at end of source. `span_check` now asserts
`Attributes::pipe` and `Attribute::span`; two minutes of `parse_utf8` clean.
Empirical diff against `dea9548` over 385 inputs: 24 938 diagnostics each
side, none gained or lost, 27 wider span ends and 2 reworded messages; USX
byte-identical. `parse_semantic` gap 10.9%: ticket 24.
