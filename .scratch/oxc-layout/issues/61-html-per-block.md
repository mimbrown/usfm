# 61. `usfm_html`: writing block by block with a shared `Context`

Status: ready-for-agent
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
