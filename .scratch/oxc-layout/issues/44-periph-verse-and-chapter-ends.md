# 44. No verse or chapter ends inside a `\periph` division

Status: needs-triage
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
