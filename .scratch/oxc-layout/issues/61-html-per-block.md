# 61. `usfm_html`: writing block by block with a shared `Context`

Status: resolved (2026-09-28)
Milestone: after M7 (publishing foundation)

HTML already allows this. `ToHtml` is implemented per block, `Context` is
public and carries the book, chapter, verse and note counters, and
`SerializeHtml` has overridable methods.

Pin it with a test: write a document's blocks one at a time with one
`Context`, wrap two of them in a caller's own `<div>`, and check that
footnote numbering carries across the wrapper. Tidy whatever the test
shows to be awkward. For example, `serialize_html` takes the sheet
separately from the document, when it should take the document's own
sheet.

## Answer

No change was needed. `crates/usfm_html/tests/footnotes.rs`'s
`blocks_written_one_at_a_time_share_the_note_numbers` writes a book one
block at a time with one `Context`, and wraps the `\m` paragraphs in the
caller's own `<section>`. The notes number 1–4 across the wrapper, and
without it the output is the whole document's, byte for byte.

One awkward spot is left as it is: `to_html_string` and `serialize_html`
take the sheet separately from the document, where they could read
`document.style_sheet()`. Changing the signatures would break every caller,
and the per-block path does not use them.
