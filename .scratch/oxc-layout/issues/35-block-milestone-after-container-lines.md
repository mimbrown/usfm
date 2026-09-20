# 35. A `Block::Milestone` after a sidebar, a table or a `\periph` line is not writable

Status: resolved
Milestone: M5
Blocked by: 27

The one `roundtrip` finding ticket 27 did not fix. Three inputs, one bug,
kept as `tasks/fuzz/findings/*.usfm` with the analysis in
`tasks/fuzz/findings/README.md`:

| Input | Where the milestone sits |
| --- | --- |
| `\periph\id\e\*` | first block of a `\periph` division |
| `\esb\c\sh\*` | after a `Sidebar` |
| `\p x`, `\tr \tc1 y`, `\c`, `\zaln-s\*` | after a `Table` |

Each parses to a `Block::Milestone` beside the blocks (in the first case the
milestone is dropped without a diagnostic, which is a hole in D1 whatever
else is decided). `usfm_codegen` writes it on a line of its own, and on the
way back the line before it — `\esbe`, a `\tr` row, the `\periph` title — runs
to the next paragraph marker and takes the milestone in, so the second tree
has it inside an implicit `\p` (or, after `\periph`, nowhere).

Ticket 27 fixed the rest of this family by reading the absorbed spelling the
way the writer's output reads: a milestone after a `Para` goes into the
paragraph (`place_block_milestone`), a `\cp` after a chapter is its published
number, a second table joins the first. This one is a rule about which blocks
may precede a `Block::Milestone` at all, and so an AST-shape statement, which
is why it was written up rather than half-fixed.

- Decide the rule and record it on `Block::Milestone`'s doc comment. The
  candidate from the findings README: a `Block::Milestone` stands between
  blocks only when the block before it ends its own line (`ChapterStart`,
  `Book`, another block milestone, or nothing); after a `Sidebar`, a `Table`
  or a `Periph`'s opening line it belongs to an implicit `\p`, as
  `place_block_milestone` already does after a `Para`. The alternative is to
  teach the writer to end those three lines (`\esbe`, the last `\tr` cell,
  the `\periph` title) with a marker that the milestone then follows on the
  next line — but there is no such marker in USFM short of a spurious `\p`,
  which is the implicit `\p` under another name, so the parser-side rule is
  the recommendation. Check what tcdocs' USX does with `<ms>` after
  `</sidebar>` and `</table>` before deciding: if a reference file has one
  there, the writer must be able to spell it and the rule needs a spelling
  (`\esbe` on its own line followed by the milestone is what the parser then
  has to read as *not* absorbed).
- The `\periph` title line: a milestone (or any non-text child) on it must
  not vanish. Either it becomes the division's first block/inline per the
  rule above, or it is dropped with a diagnostic (`content-after-marker` or
  a new `Code`, with a `recovery.rs` test either way).
- Tests: the three inputs in `crates/usfm_codegen`'s
  `roundtrip.rs::the_fuzz_findings_round_trip`, one `recovery.rs` test per
  placement (snapshot), and a USX test if the tree changes for a valid
  document.
- Then delete `tasks/fuzz/findings/` (the README says a finding leaves when
  it becomes a test and a fix; recreate the directory only when there is a
  next open finding), update `tasks/fuzz/README.md`'s table, `./seed.sh
  --prune`, and rerun `cargo +nightly fuzz run --fuzz-dir tasks/fuzz roundtrip
  -- -max_total_time=600 -max_len=65536` until it is ten minutes clean from
  the seeds. Every new finding on the way follows ticket 06's rules (test
  first, then fix, or `findings/` with a note).
- Record the run in `tasks/fuzz/README.md`'s table and note in the spec's M5
  exit record that the criterion ticket 27 left open is met.

Done when the gate is green, the three inputs round-trip, `findings/` is
empty or gone, and `roundtrip` has run ten minutes clean.

## Answer

Landed in `beac640` (PR #34, 2026-09-20). No reference file in either conformance
root puts an `<ms>` after `</sidebar>` or `</table>` or first in a periph
(the only block-level `<ms>` are `usfmjsTests/ts` and `ts_2`, after a
`<chapter>`), so the rule from the findings README is adopted and recorded
on `Block::Milestone`: one stands between blocks only when the block before
it is one the writer gives a line of its own (`Book`, `ChapterStart`,
`ChapterEnd`, another block milestone, or nothing at the head of the
document's list); after a `Para`, `Table`, `Sidebar` or `Periph`, and at the
head of a sidebar's or periph's own list, it belongs to an implicit `\p`.
`place_block_milestone` implements it (a `content-outside-paragraph` naming
the written form's line, `\esbe`/`\tr`/`\periph`, the same code the
`\esbe`-then-milestone spelling already reported), and `parse_blocks` carries
a `BlockListHead` for the head case. The `\periph` title line keeps only its
text as the title and gives everything else (a milestone, a style, a note, a
`\v`) to the division's implicit `\p` through `place_periph_title_leftovers`
instead of dropping it silently (the D1 hole); `\periph T` + `\qt-s\*` and
`\periph T` + `\p \qt-s\*` are one tree. One existing expectation changed
for that reason: `verse_ends.rs::a_verse_on_a_periph_line_opens_no_verse_end`
keeps the `VerseStart` (still no end: verses stay suspended across a periph).

The next run found one more: `\v 1\vp\`, a published verse number written
raw whose `\` ran into the `\vp*` after it. `\vp`'s number is now written as
text; `\cp`'s stays verbatim (its reader takes one `Word` token), and the
`\cp`-paragraph fold of ticket 27 gives up a first word only when a `Word`
token could be it. Test: `usfm_codegen`'s
`a_published_number_is_written_the_way_its_marker_reads_it`.

`roundtrip` then ran ten minutes clean from the pruned seeds (56 987 runs in
601 s, 94 exec/s, cov 4636, no crash), recorded in `tasks/fuzz/README.md`;
`tasks/fuzz/findings/` is deleted; the committed corpora are the 295 seeds
again (ticket 27 had committed 1634 grown inputs under `corpus/roundtrip/`).
New snapshots: `recovery__block_milestone_after_a_sidebar`, `…_after_a_table`,
`…_at_the_head_of_a_periph`, `…_on_a_periph_title_line`;
`usx_text.rs::a_milestone_on_a_periph_title_line_reaches_the_output`. Gate
exit 0. The spec's M5 section records that ticket 27's open criterion is met.
