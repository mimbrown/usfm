# 42. A USX reader into `Document`, and the USX -> USFM -> USX round trip

Status: needs-triage
Milestone: after M6

Nothing reads USX back into a `Document`. `usfm_usx` has an `XmlDocument`
reader (the conformance harness parses reference files with it to
compare), but no `XmlDocument -> Document`, so the round trip the hardening
plan first asked for — USX -> USFM -> USX — cannot be checked, and a
pipeline that starts from USX (Paratext exports, the tcdocs references
themselves) has no way in. `docs/plans/hardening.md`, "Property tests",
records the gap.

This is a new crate-sized piece of work, not a follow-up: a reader that
maps `<para>`, `<char>`, `<note>`, `<ms>`, `<chapter>`/`<verse>` with
`sid`/`eid`, `<table>`, `<sidebar>`, `<periph>` and `<optbreak/>` onto the
AST (spans would be `SPAN` or the XML offsets), a `StyleId` per `style`
attribute through the stylesheet, and the reverse property over the
conformance references. Needs a spec section and tickets of its own before
an unattended loop could take it; write those first.

Done when the spec has an M7 (or whatever it is called) with exit criteria.
