# 14. `usfm_html`: HTML output as its own crate

Status: ready-for-agent
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
