# 36. `\xt ...\xt*` after `\xo` is read as nested inside the `\xo`

Status: resolved
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

## Comments

The evidence narrows the rule: it is about `\xo`, not about note-internal
styles in general.

For the sibling reading, all three in `paratextTests`:

- `NestingInCrossReferencesInvalid` (`\x + \xo 1.1 \em \+pn name\+pn* stuff
  \em*\x*`) states it in its own `metadata.xml` — "Grammar is accepting nesting
  of character styles under `\xo` - this is normally just text" — and its
  reference USX closes the `\xo` at the `\em`. Its patch *was* hiding this: the
  accepted-deviation half is gone, and what is left is the `status="invalid"`
  attribute, Paratext's reporting channel.
- `CrossReferencesQuoteOutsideNote` (`\xo 1.1 \em stuff\em*`) and
  `CrossReferencesInsideCharacterMarker` (`\xo 1.1 \xq stuff\xq*`) read the
  same way. No reference file in either root puts a `<char>` inside a
  `<char style="xo">`.

Against the wider rule the ticket guessed at ("a note-internal style does not
nest in another note-internal style"):

- `biblica/CategoriesOnNotes` (a `pass` case) nests a *closed* `\xt` inside
  `\ft` and a `\ref` inside that `\xt` — both note-internal both times;
- `specExamples/extended/contentCatogories1` nests `\sc BC\sc*` inside `\ft`;
- `usfmjsTests/usfmBodyTestD` nests `\dc` inside `\ft` and inside `\xt`.

Adopting the wider rule would have flattened all of those and made three `pass`
cases match their reference worse, so it was not adopted.

`usfm.sty`: `\xt` is the only marker with `\TextType NoteText` and `NEST` in
its `\OccursUnder` (`\ref`, which `build.rs` adds, is the other nestable
note-internal style). `\xt`'s `\OccursUnder` lists paragraph styles plus
`f fe ef efe x ex` and `NEST` — neither `xo` nor `ft`, so it separates nothing;
`tcdocs/grammar/usx.rnc` does not separate them either (`CrossReferenceChar`
and `FootnoteChar` both admit `NoteCharEmbed`, and `xt` is in
`Char.char.style.enum`). The reference USX and that one `metadata.xml` are the
whole of the evidence, and they name `\xo`.

`omitted_closers`' `!nest || omitted[i+1]` clause therefore stays: with the
rule scoped to `\xo`, a closed `\xt` after `\fq`/`\ft`/`\xq` still nests, which
is what `a_style_before_a_closed_xt_keeps_its_closer` covers (it uses `\xq`).
The clause is now one case wider than the parser needs — an `\xo` before a
closed `\xt` could drop its `\xo*` too — and the writer keeps the `\xo*`,
which is recorded on `omitted_closers`: nothing in either corpus writes that
shape, and `\xo*` reads back to the same tree, so the output is still a fixed
point.

## Answer

Landed via PR #35 (2026-09-20). The evidence went against the rule the
ticket proposed and for a narrower one. Three `paratextTests` references
read a closed style after `\xo` as its sibling (`NestingInCrossReferencesInvalid`,
whose `metadata.xml` says "Grammar is accepting nesting of character styles
under `\xo` - this is normally just text"; `CrossReferencesQuoteOutsideNote`;
`CrossReferencesInsideCharacterMarker`), and no reference puts a `<char>`
inside a `<char style="xo">`. But `biblica/CategoriesOnNotes` nests a closed
`\xt` inside `\ft` (and a `\ref` inside that), `specExamples/extended/contentCatogories1`
nests `\sc` inside `\ft`, and `usfmjsTests/usfmBodyTestD` nests `\dc` inside
`\ft` and `\xt` — so "note-internal never nests in note-internal" would have
moved three `pass` cases away from their references. `usfm.sty` separates
nothing (`\xt`'s `OccursUnder` lists neither `xo` nor `ft`).

The rule is therefore a third condition on the nesting decision in
`parse_char`: the style being nested *into* must not be `\xo`
(`ParserImpl::parent_holds_plain_text`, with the case names on it). `\+xt`
still nests. The `NestingInCrossReferencesInvalid` patch had been forcing the
reference into the nested reading; that half is gone and the patch now only
strips Paratext's `status="invalid"` (still fourteen patches). One tcdocs
snapshot moved to match its reference (`CrossReferencesQuoteOutsideNote`:
`\em` beside `\xo`, no `character-style-nested-without-plus`). Conformance
unchanged at 231 / 0 / 44, baseline empty; round trip 275 / 275; seed scan
clean; `format --check` still exits 0 on the corpus.

The writer keeps `omitted_closers`' `!nest || omitted[i+1]` clause: a closed
`\xt` after `\fq`/`\ft`/`\xq` still nests, so the clause is still
load-bearing (one case wider than needed — an `\xo*` before a closed `\xt`
could go — documented rather than special-cased; no corpus writes it).
Tests: `recovery.rs::closed_nestable_style_after_xo_is_a_sibling`,
`a_plussed_style_still_nests_inside_xo`, `closed_style_after_fr_is_a_sibling`;
`usx_text.rs::a_closed_xt_after_xo_is_a_sibling_in_usx`,
`a_closed_style_inside_ft_still_nests_in_usx`; `usfm_codegen`'s
`a_closed_xt_after_xo_is_written_as_a_sibling`, `a_plussed_xt_inside_xo_stays_nested`.
