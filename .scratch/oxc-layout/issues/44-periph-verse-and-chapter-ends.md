# 44. No verse or chapter ends inside a `\periph` division

Status: resolved
Milestone: after M6

Found by ticket 38. A `\c` and `\v` written inside a `\periph` division get
their starts and no ends:

```
\id FRT
\periph Title|id="title"
\c 1
\p \v 1 a
\c 2
\p \v 1 b
```

writes

```xml
<periph alt="Title" id="title">
  <chapter number="1" style="c" sid="FRT 1" />
  <para style="p"><verse number="1" style="v" sid="FRT 1:1" />a</para>
  <chapter number="2" style="c" sid="FRT 2" />
  <para style="p" vid="FRT 2:1"><verse number="1" style="v" sid="FRT 2:1" />b</para>
</periph>
```

— no `<verse eid>`, no `<chapter eid>`, and a `vid` on a paragraph that opens
with its own `\v`. Ticket 27 suspends verses across a periph line (so a `\v`
on the title line is not lost) and plan D7 keeps ends out of sidebars; this
looks like the periph case of that rule reaching further than it should,
since `usx.rnc`'s `PeripheralContent` allows `Chapter` and every other
chapter in USX is closed. No conformance case has a `\c` inside a
division, which is why nothing caught it.

- Decide the rule: ends inside a division behave as they do at top level,
  bounded by the division (a verse open at the next `\periph` ends there).
- A test per shape in `crates/usfm_parser/tests/verse_ends.rs`; the
  `vid` case in the USX writer's tests.

Done when the example above writes one `eid` per `sid` and no `vid` on the
second paragraph, and the gate is green.

## Answer

Picked by Michael on 2026-09-26. The rule is the one proposed above, and it
amends plan D7 ("peripheral matter has no verses"), which now says so.
`parse_periph` no longer suspends verse tracking. Before the `\periph` line
it ends the open verse (in the last verse-text block before the division)
and the open chapter, since nothing written after that line is outside the
division; when the division ends it ends whatever the division opened,
inside it. Sidebars still suspend tracking, and a periph inside a sidebar
tracks nothing, as before.

Tests: `verse_ends.rs`'s `chapters_and_verses_inside_a_periph_division_are_closed`
and `a_periph_division_ends_the_verse_and_chapter_before_it`; the old
`a_verse_on_a_periph_line_opens_no_verse_end` is now
`a_verse_on_a_periph_line_ends_with_the_division` (both spellings still agree);
`usx_text.rs`'s `chapters_inside_a_periph_division_are_closed` pins the `eid`s
and the missing `vid`; `recovery__block_after_a_periph_that_nothing_ended`
gains its `VerseEnd`. No conformance case has a `\v` in a division, so tcdocs
is unchanged.
