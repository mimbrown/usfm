# 36. `\xt ...\xt*` after `\xo` is read as nested inside the `\xo`

Status: ready-for-agent
Milestone: M5
Blocked by: 34

Found by ticket 34. `\x - \xo 1.1 \xt Gen 1.1\xt*\x*` parses with the `\xt`
as a child of the `\xo` rather than its sibling, and `usfm_codegen` then
writes `\xo 1.1 \+xt Gen 1.1\+xt*`. The nesting rule (CLAUDE.md, "Character
nesting without `\+` follows Paratext") says a style nests iff its entry
allows `NEST` *and* its own closing marker lies ahead in the container; `\xt`
is the only note-internal marker with `NEST`, and here its `\xt*` lies ahead,
so the rule fires. But `\xo` is a note *origin* whose content is a reference,
and `\xt` after it is the note's next run, closed or not: no reader of that
line sees a nested style, and the closed spelling is common (Paratext writes
`\xt` with attributes closed, `\xt ...|link-href="…"\xt*`).

- Check what tcdocs' reference USX does for a closed `\xt` after `\xo` (and
  for `\ft ... \+xt ...\+xt*`, the spelling that really means nesting): grep
  `tcdocs/tests/*/*/origin.usfm` for `\xt` followed by `\xt*` inside `\x`,
  and the usfm-grammar and machine.py fixtures. If a reference file has the
  sibling reading, the harness is already telling us and a patch is hiding it
  (check `tasks/conformance/tcdocs-patches/`); if none has the shape, decide
  from Paratext's behaviour (`usfm.sty`'s `OccursUnder` for `\xt` — does it
  list `xo`?) and record the decision on `char_is_closed_ahead`.
- The likely rule: inside a note, a note-internal style does not nest in
  another note-internal style without `\+`, whatever lies ahead; `\+xt` still
  nests. If adopted: `recovery.rs` test (snapshot), `usx_text.rs` test, and
  the writer's `a_style_before_a_closed_xt_keeps_its_closer` simplifies
  (the `!nest || omitted[i+1]` clause in `omitted_closers` becomes
  unnecessary — remove it and its doc, keep the test as proof).
- tcdocs must stay at 215 / 0 / 44 and usfm-grammar 16 / 0; if a case
  changes, the reference file is the arbiter, never a patch.

Done when the decision is recorded, the tests pin it, and the gate is green.
