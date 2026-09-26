# 42. A USX reader into `Document`, and the USX -> USFM -> USX round trip

Status: resolved
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

## Answer

Specified 2026-09-26 (Michael, in the project thread: tests first, then
"third-party crate or our own?"; answered and "Agreed, proceed"). The spec
has an **M7. USX reader** section with exit criteria and the decisions a
loop should not reopen, and M7 is ticketed as 45–49.

- Tests: the conformance roots already pair every USFM input with its USX
  (258 tcdocs `origin.xml`, 13 usfm-grammar), CC BY 4.0 and MIT and already
  in `NOTICE.md`. Their USX was all generated from USFM, so real exports come
  from sillsdev/machine.py (MIT): the public-domain WEB as DBL writes it, and
  a malformed USX 2.6 test book (ticket 47).
- XML: `roxmltree`, not our own parser and not `xml-rs` for the reader —
  byte ranges for spans, no `unsafe`, MIT OR Apache-2.0 with both licence
  files. The reasoning is in the spec.
