# 57. A publishing layer: volumes, output models, scoped edits, media

Status: resolved (2026-09-28)
Milestone: after M7 (render gaps)

Found by mapping Shahkar-Urdu-Apps/render (gaps 5, 7, 8, 9). What render does
between the parse and its outputs, none of which fits the frozen AST and
all of which it does by grafting non-USFM nodes onto its own tree:

- **Volumes** (`render:products.json`): books from several projects, chapters
  picked, reordered and renumbered (`SSV.INT 22, 13, 19, 14` is BAK), one
  project's books in another's volume, links resolved across volumes.
- **Output-only nodes**: `book:title` (the `\mt1`–`\mt3` group), `bismallah`
  (`\mt4`), a timeline built from the table between `\timeline-start` and
  `\timeline-end` milestones, a table as an image, the chapter drop number,
  per-chapter paragraph and verse indexes and footnote numbers.
- **Structural edits**: hoisting a figure at a paragraph's edge to block
  level, moving a verse that opens a paragraph into the heading before it,
  dropping empty paragraphs and styles.
- **Scoped, per-medium text edits**: font workarounds applied only inside
  some styles, only outside notes, only after the first `\c`, only for the
  digital medium; some inserted *next to* a verse end or a note, which
  render does with regexes over its serialised JSON.
- **SILE**: render's typesetter wants `<vs start end>`/`<verse start end>`,
  separate interlinear / literal / main streams per chapter, and the
  introduction split from the main text. Ours writes USX under a `<sile>`
  root.

**The question** is the design, and how much of render should move here at
all. The shape that fits the ADR is a render model built *from* a
`Document` (or several) in `usfm_pipeline` or a new crate — not new AST
variants — with volumes as a collection of documents plus chapter selection,
and edits scoped by a small selector (style, ancestor, medium). That needs a
spec section and Michael's view of which outputs matter, before any ticket
under it is ready for an agent.

## Answer

Michael, 2026-09-28. usfm is a foundation to build on, not a new home for
render. render has five layers: the base USFM; edits shared by every
output; the split into print and digital; the book pipeline; and the
digital pipeline. Output-only structure, such as a cartouche over several
`\m#` paragraphs, lives in render's own model (`RenderBlock`) over usfm's
`Block`s. Doing that must not mean re-implementing the writers.

render moves to Rust end to end. The design is the spec's "Publishing
foundation" section: no AST variants; writers you drive one block at a time,
keeping their state between blocks, with hooks to override one node kind.
It is split into tickets 58–64.
