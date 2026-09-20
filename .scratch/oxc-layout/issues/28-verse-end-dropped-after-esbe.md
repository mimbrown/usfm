# 28. A verse end is dropped when `\v` follows `\esbe` with no paragraph marker

Status: resolved
Milestone: M5

Found by ticket 25's round trip on machine.py's `41MATTes.SFM`, the one
input in the repo that does not round-trip. Minimal input:

```
\id GEN
\c 1
\p
\v 1 a
\esb
\p side
\esbe
\v 2 b
```

gives `<para style="p"><verse sid="GEN 1:1"/>a</para>` with no
`<verse eid="GEN 1:1"/>`, and the paragraph after the sidebar wrongly carries
`vid="GEN 1:1"`. With a `\p` after `\esbe` the `eid` is placed correctly.

Cause (`crates/usfm_parser/src/parser.rs`, `parse_sidebar`): the `\esbe`
line is parsed with `parse_paragraph(&mut tail, esbe, span)` into a local
`tail`, the pending verse end is handed to
`place_pending_verse_end_before_block(&mut tail)`, `tail` is empty, and
`end_verse_in_last_block` drops it silently.

- Fix so the verse end lands before the sidebar (the D7 rule: verse ends
  skip sidebars) whether or not a paragraph marker follows `\esbe`.
- Test in `crates/usfm_parser/tests/verse_ends.rs` (the placement rules are
  one test each there) and a USX test.
- Then `41MATTes.SFM` round-trips: add it to the round-trip test's inputs
  (the machine-py fixtures are not a harness root; the round-trip test can
  list them) and confirm.

Done when the gate is green and the round-trip test covers the machine-py
fixtures.

## Answer

Fixed by ticket 27 (PR #32, 2026-09-20), where the `roundtrip` fuzz target's
seed scan hit `41MATTes.SFM` first. `parse_sidebar` pushes the sidebar
*before* parsing the `\esbe` line, so the verse end is handed to the real
block list and lands before the sidebar (D7), and the `\esbe` line places
verse ends as the implicit `\p` it becomes (`parse_paragraph_as`). Tests:
`verse_ends.rs::a_verse_after_esbe_with_no_paragraph_marker_ends_the_one_before_the_sidebar`,
`usx_text.rs::a_verse_after_esbe_closes_the_verse_before_the_sidebar`; the
machine-py fixtures are in `usfm_codegen`'s
`roundtrip.rs::the_machine_py_fixtures_round_trip`.
