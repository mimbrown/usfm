# 38. `ReferenceIndex` does not see the chapters of a `\periph` division

Status: resolved
Milestone: after M6

Found by ticket 32. `usfm_semantic::ReferenceIndex::new` reads chapters
from the document's top-level blocks only. A `\periph` division runs to the
next `\periph` or `\id` (ticket 27), so in a front-matter book every `\c`
after the periph is *inside* `Block::Periph` and the index has no chapter at
all. The language server's outline worked around it with its own walk
(`apps/usfm_language_server/src/symbols.rs`); nothing else has hit it yet
because the benchmark corpus and the conformance roots have no `\periph`
followed by chapters.

- Decide whether the index should descend into `Periph` (and `Sidebar`,
  which the D7 rule keeps verse ends out of) or whether a periph's chapters
  are by definition not the book's. The USX side: `<periph>` may hold
  `<chapter>` per `usx.rnc`; check.
- If it descends: `NodePath`s already reach inside containers, so the change
  is in the walk that builds `chapters`; a test in
  `crates/usfm_semantic/tests/` with `\periph` + `\c 1` + `\v 1`, and the
  server's `symbols.rs` may then use the index (say whether it should — it
  also needs the containers themselves, which the index does not model).

Done when the decision is recorded on `ReferenceIndex`'s doc and pinned by a
test either way.

## Answer

Picked by Michael on 2026-09-26. **The index descends into `Periph`, and
not into `Sidebar`.** `usx.rnc`'s `PeripheralContent` lists `Chapter`, so
a `\c` inside a division is one of the book's chapters; a sidebar's
content (`Section | Para | List | Table | Footnote | CrossReference`)
allows none. `ChapterEntry` records which container it is in (`periph:
Option<usize>`, the division's top-level index), `ChapterRef::start` and
`blocks` read from that container, and a division written after a `\c`
ends that chapter, since everything after `\periph` is the division's. The
decision is on `ReferenceIndex::new`'s doc, pinned by
`chapters_inside_a_periph_division_are_indexed` and
`a_periph_division_ends_the_chapter_before_it` in `reference.rs`.

`symbols.rs` keeps its own walk: the outline needs the containers
themselves, which the index does not model. Its module doc now says so and
no longer cites this gap.

Found on the way: inside a division the parser emits no verse or chapter
ends, and puts a `vid` on a paragraph that opens with its own `\v`. That is
ticket 44.
