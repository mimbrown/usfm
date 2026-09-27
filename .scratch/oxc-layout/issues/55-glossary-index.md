# 55. A glossary index: which `\k` entry a `\w` word refers to

Status: ready-for-agent
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
