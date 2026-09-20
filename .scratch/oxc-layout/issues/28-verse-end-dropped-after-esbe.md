# 28. A verse end is dropped when `\v` follows `\esbe` with no paragraph marker

Status: ready-for-agent
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
