# 55. A glossary index: which `\k` entry a `\w` word refers to

Status: resolved
Milestone: after M7 (render gaps)

Found by mapping Shahkar-Urdu-Apps/render (gap 6). render links every `\w`
in the text to the `\k` keyword that defines it in the GLO book, across
books and volumes (`render:lib/bookLoader.ts:202-236`), and rewrites GLO with
regexes before parsing to make each entry addressable. We parse both markers
already; what is missing is the index.

- `usfm_semantic::GlossaryIndex`, in the style of `ReferenceIndex`: built
  from one or more documents (the glossary may be its own book), one entry
  per `\k` with its key text (plain text, whitespace collapsed), the book,
  the chapter it is in, the block that holds it and its span.
- `lookup(word)` for a `\w` node: its `lemma` attribute if it has one, else
  its plain text, matched against the keys after the same normalisation;
  and `unresolved(&document)` lists the `\w` words with no entry.
- The first entry for a key wins, as in `ReferenceIndex`; a key defined
  twice is reported through `duplicates()` rather than a diagnostic (a
  semantic check would need the glossary in the same document).
- Re-exported on the facade as `usfm::GlossaryIndex`.

## Answer

`usfm_semantic::glossary` (`usfm::GlossaryIndex` on the facade).
`GlossaryIndex::new(&[&Document])` indexes every `\k` of the documents
given, in order; each `GlossaryEntry` has its term (whitespace collapsed),
the document's position in the slice, its book, its chapter and the `\k`
node's span. `get(term)`, `words(&document)` (every `\w`, its term — the
lemma, written as `lemma="…"` or as the default attribute, else its text —
and its entry), `unresolved(&document)` and `duplicates()`. Terms compare
after `normalise`: whitespace collapsed and lower-cased (a choice render
does not make — it compares exactly — taken so `Grace` in a glossary
matches `\w grace\w*`; the case fold is Unicode's, a no-op for Urdu).

render rewrites its GLO book with regexes before parsing (`\s` to a fake
`\c` + `\imt`) to make entries addressable; with this index an entry is
addressed by its chapter and span as the parser read it.

Tests: `crates/usfm_semantic/tests/glossary.rs` (a glossary book and a
text in two documents, lemma forms, a repeat).
