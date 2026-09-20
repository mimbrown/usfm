# 34. Codegen: idiomatic note spelling

Status: resolved
Milestone: M5
Blocked by: 27

Found by ticket 26: `usfm format --check` reports 78 of the 86 WEB books
because the writer closes every note-internal character style explicitly
(`\f + \ft text\ft*\f*`) where every USFM writer, Paratext included, writes
`\f + \ft text\f*`, and it writes `\cp 151` after `\c 151` on the same line
where the corpus has it on its own line. Both are legal spellings and the
round trip holds; the formatter's shape is just not the shape anyone writes.

- Rule for note-internal styles (`\fr`, `\ft`, `\fq`, `\fqa`, `\fk`, `\fl`,
  `\fv`, `\fw`, `\fp`, `\xo`, `\xt`, `\xk`, `\xq`, `\xta` and their `\e`
  twins: the stylesheet knows them as `TextType NoteText`): omit the closing
  marker when the next sibling in the note is another note-internal style or
  the note's own closer; write it when the next sibling is text (`\fq b\fq*
  trailing`), a nested style, or anything else. The parser's implicit-close
  rule (`character-style-implicitly-closed` is Info for these) must read the
  omitted form back to the identical tree; the round-trip test proves it.
- `\cp` and `\ca`: on the `\c` line (Paratext writes `\c 1 \ca 2\ca*`? check
  the spec and tcdocs' `pass` inputs for the common spelling; follow the
  majority spelling in tcdocs).
- `usfm format --check tasks/benchmark/corpus/web/*.usfm` then exits 0 (the
  criterion ticket 26 could not meet); if a file still differs, diff it and
  decide the same way: canonical where USFM has one spelling, the corpus's
  where it is the idiom.
- `crates/usfm_codegen/tests/roundtrip.rs` and the fuzz round trip from
  ticket 27 must stay green.

Done when the gate is green and the corpus `--check` passes.

## Answer

Landed via PR #33 (2026-09-20). Both spellings changed in
`crates/usfm_codegen/src/usfm.rs`, neither a tree change:

- A note's content runs are written without their closing marker
  (`\f + \fr 1.1 \ft the note\f*`). Which styles are note-internal is the
  stylesheet's answer (`is_note_text`: a character style whose `TextType` is
  `NoteText`, 22 markers in `usfm.sty` plus `usfm-extra.sty`, derived styles
  included), not a list. `omitted_closers` decides a note's children from the
  right: the closer goes when the next sibling is another note-internal style
  or the note's own closer, and stays before text, a milestone, an outside
  style, or when the style carries an attribute list. One clause the ticket
  did not have and the tests forced: `\xt` is the only note-internal marker
  with `NEST`, and the parser nests an unmarked `\xt` when its `\xt*` lies
  ahead, so a sibling in front of a *closed* `\xt` keeps its closer or the
  `\xt` becomes its child. The parser reports nothing for an implicit close
  inside a note, so the second parse gains no code.
- `\ca` and `\cp` go on lines of their own after `\c N`: the spec's own
  example (`specExamples/chapter-verse`) and every corpus in the repo spell
  them so (own-line 4 : same-line 1, and the one same-line case is an
  usfm-grammar autofix input).

`usfm format --check tasks/benchmark/corpus/web/*.usfm` exits 0 on all 86
books (78 differed before, in exactly those two classes: 4122 note closers
and one `\cp`); `cli.rs::check_passes_on_the_whole_benchmark_corpus` guards
it in under a second. Six new writer tests (`note_internal_styles_run_into_one_another`,
`..._followed_by_anything_else_keeps_its_closer`, `..._with_attributes_keeps_its_closer`,
`a_style_before_a_closed_xt_keeps_its_closer`,
`a_nested_style_inside_a_note_keeps_its_closer`,
`a_note_internal_style_outside_a_note_keeps_its_closer`). Round trip green
on every corpus, the fuzz seed scan clean, gate exit 0.

Follow-up: ticket 36 — the parser reads `\x - \xo 1.1 \xt Gen 1.1\xt*\x*`
with the `\xt` nested *inside* `\xo` (its `\xt*` lies ahead and `\xt`
allows `NEST`), which is not how a reader of that USFM sees it; the writer
faithfully emits `\+xt`. Pre-existing, found here.
