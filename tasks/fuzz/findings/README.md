# Findings not yet fixed

One file per open finding, with the input exactly as `cargo fuzz tmin`
minimised it, and a section here saying what the property was, why it fails and
what fixing it would take. A finding leaves this directory when it becomes a
test and a fix (`../README.md`); nothing here is a licence to ignore it.

## A `Block::Milestone` the writer has no line for

Three inputs, one bug (ticket 27, `roundtrip`):

| File | Input | Where the milestone sits |
| --- | --- | --- |
| `periph-title-line-swallows-a-block-milestone.usfm` | `\periph\id\e\*` | first block of a `\periph` division |
| `block-milestone-after-a-sidebar.usfm` | `\esb\c\sh\*` | after a `Sidebar` |
| `block-milestone-after-a-table.usfm` | `\p x`, `\tr \tc1 y`, `\c`, `\zaln-s\*` | after a `Table` |

Each parses to a `Block::Milestone` standing beside the blocks, which is what
USX does with one (`<ms>` next to `<para>`), and each is written back by
`usfm_codegen` on a line of its own — where the *previous* line reaches out and
takes it:

```
\esb\c\sh\*        parses to   Sidebar[], Milestone sh
                   writes      \esb / \esbe / \sh\*
                   reparses to Sidebar[], Para p[Milestone sh]
```

`\esbe`, a `\periph` title and a `\tr` row all run to the next *paragraph*
marker, and a milestone is not one, so on the way back in the milestone is read
as part of that line. (`\periph`'s title line is worse: it keeps only the text
of the line, so the milestone is dropped without a diagnostic — a hole in
"recovery is never silent", `docs/plans/hardening.md` D1, whatever is decided
here.)

Ticket 27 fixed every other member of this family by making the parser read the
absorbed spelling the way the writer's output reads: a milestone after a
paragraph goes *into* the paragraph, a `\cp` after a chapter becomes its
published number, a second table joins the first. The same move works here —
the milestone goes into an implicit `\p`, which is exactly what the re-parse
makes of it — but it is a rule about **which blocks can precede a
`Block::Milestone` at all**, and that is an AST-shape statement rather than a
repair at one site. The rule would be:

> A `Block::Milestone` is only a block when the block before it is one whose
> written form ends its own line: a `ChapterStart`, a `Book`, another block
> milestone, or nothing. After a `Sidebar`, a `Table` or a `Periph`'s opening
> line it belongs to an implicit `\p`, as `place_block_milestone` already does
> for a `Para`.

Settle that (and what `Periph::title` means when its line held more than text)
before implementing it, because it decides what a `Block::Milestone` *is*.
That decision is ticket 35 (`.scratch/oxc-layout/issues/35-…`).

Reproduce:

```bash
for f in tasks/fuzz/findings/*.usfm; do
  cargo +nightly fuzz run --fuzz-dir tasks/fuzz roundtrip "$f"
done
```
