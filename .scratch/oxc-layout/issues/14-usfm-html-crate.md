# 14. `usfm_html`: HTML output as its own crate

Status: resolved
Milestone: M3
Blocked by: 13

`usfm_parser/src/serialize_html.rs` (561 lines, `ToHtml`, `SerializeHtml`,
`to_html_string`, `serialize_html`, `XmlAttributes`) and `serialize.rs` (298
lines, the generic `Serialize` trait the SILE output and
`examples/custom_html_serialization.rs` use) move to `usfm_html`.

- New crate `usfm_html` depending on `usfm_ast`, `usfm_style`. If
  `usfm_parser/src/context.rs` is still needed after ticket 13, it moves here
  as `usfm_html::Context` (note numbering and custom counters are HTML
  concerns: footnote markers, ids); if 13 found nothing else uses it, delete
  the parser's copy.
- `SerializeHtml`'s customisation hooks (`serialize_text`, `serialize_book`,
  `serialize_char`, …) are kept: the example and `main.rs`'s
  `DocumentSectionHtmlSerializer` depend on them. The example moves to
  `usfm_html/examples/`.
- The HTML tests in `serialize_html.rs` (`mod tests`) move with it and stay
  green; add one test that `to_html_string` over
  `tasks/benchmark/corpus/web/71-WIS.usfm` numbers footnotes 1..268 in order.
- Ticket 06 found that the USX writer let C0 controls through; the HTML
  writer has the same hole. Replace the characters HTML cannot carry the way
  `xml_document.rs` does (byte-table scan, U+FFFD), add a `parse_html` fuzz
  target in `tasks/fuzz` that checks the output is well-formed (a small
  tag-balance check is enough; no HTML parser dependency), and run it 10
  minutes clean.
- `usfm_parser::serialize_html` / `serialize` become re-exports for one
  milestone.
- Bench: `parse_html` within 3% (interleaved), recorded.

Done when the gate is green and the bench is within 3%.

## Answer

Landed via PR #17 (2026-09-19). `usfm_html` (depends on `usfm_ast` and
`usfm_style`; `usfm_parser` only as a dev-dependency for tests) holds
`serialize_html.rs`, `serialize.rs`, `context.rs` (now `usfm_html::Context`,
minus the dead `from_book`/`marker`; the spec's `Context` question is settled:
it survives as HTML-only state), the example and its doc. `usfm_parser::
serialize_html`/`serialize`/`context` re-export it until ticket 17.

Found on the way: the HTML writer escaped nothing at all, not even `&`, `<`,
`>`. Every document-derived string now goes through `write_escaped` /
`write_escaped_attribute` (`escape.rs`, the same byte-table scan as the USX
writer, copied on purpose: no output crate depends on another), forbidden
control characters become U+FFFD, and a milestone attribute with an invalid
name is dropped as in USX. `tests/footnotes.rs` pins the 268 footnote numbers
of `71-WIS.usfm` and the two-counter behaviour of custom callers
(`note-c1`…). New `parse_html` fuzz target with a tag-balance and entity
scanner: two 10-minute runs, no findings. `parse_html` interleaved against
`d918bd4`: whole-corpus 39.92 → 38.84 MiB/s (−2.7%), all classes within 3%;
the escaping costs 2–3% on text-heavy classes, recorded in
`docs/benchmarks.md` "After ticket 14". Miri runs the `escape` tests (the
whole lib is 15 s, over budget).

Follow-ups, on ticket 15: the diglot section serializer in `main.rs` writes
raw text and must use `write_escaped`; the `Serialize` trait has no
implementor.
