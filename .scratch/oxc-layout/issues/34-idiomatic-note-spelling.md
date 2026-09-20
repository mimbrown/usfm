# 34. Codegen: idiomatic note spelling

Status: ready-for-agent
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
