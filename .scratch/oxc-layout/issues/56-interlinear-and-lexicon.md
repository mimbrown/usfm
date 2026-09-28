# 56. Paratext interlinear glosses and lexicon

Status: resolved (2026-09-28)
Milestone: after M7 (render gaps)

Found by mapping Shahkar-Urdu-Apps/render (gap 4). render prints and shows
the Greek and Hebrew source text (the ALEPH and LCx projects) with an Urdu
gloss under each word: `Interlinear_ur_<BOOK>.xml` gives, per verse, clusters
with a word index and a lexeme, and `Lexicon.xml` maps the lexeme's sense to
its gloss (`render:lib/getInterlinear.ts`, `getLexiconMappings.ts`). Its
alignment is positional (the i-th gloss goes to the i-th whitespace-separated
word) with footnote words skipped by counting. It also pairs a second USFM
project, the literal translation, with the main text verse by verse, merging
verses when the main text writes a range.

**The question** is the representation. We have no reader for either XML
file and no model of "text B aligned to text A". Options:

- (a) read them into the source text as USFM 3 word attributes,
  `\w logos|x-gloss="…"\w*` (or `\zaln-s` / `\zaln-e` milestones), which our
  AST already holds and round-trips — the unfoldingWord shape of the
  `aligned` benchmark class — so interlinear is a reader that returns an
  aligned `Document`, not a new AST concept;
- (b) a side table keyed by verse and word index, outside the tree;
- (c) leave it to the consuming project.

Recommendation: (a), with alignment by Paratext's character ranges
(`Range@Index`/`Length`) rather than word position. Needs a real project's
interlinear and lexicon files to test against, which only Michael can
supply (they are not in render's repository; its CI syncs them from the
Paratext server).

## Answer

Michael, 2026-09-28: "we do need to handle interlinear", and asked for
open-source samples. SIL's interlinearizer extension (MIT) has four
invented Paratext 9 projects with every file and a written spec of the
format (`src/parsers/pt9/pt9-xml.md`); they are vendored as
`crates/usfm_paratext/tests/fixtures/pt9/` (`NOTICE.md`). No openly
licensed real-language interlinear exists that we found.

The spec settles the representation, and it is **not** (a): a cluster's
`Range` counts characters of Paratext's own verse string (which carries the
verse marker), Paratext never rewrites ranges when the text changes, and it
matches an analysis to a word by the lexeme's *form*. So a range cannot be
written into the tree as an alignment; it is ordering and a tie-breaker. The
answer is (b), a side structure, placed on the text by form:

- `usfm_paratext::InterlinearBook` reads `Interlinear_{lang}_{book}.xml`
  (verses, clusters, lexemes with their chosen sense, `Excluded`, the
  approval hash, punctuation changes), `Lexicon` reads `Lexicon.xml`
  (entries, senses, glosses per language, the legacy analyses), and
  `Project::interlinear` / `interlinear_languages` / `lexicon` find them
  (the Paratext 9 folder first, the older top-level file second).
- `usfm_paratext::anchor::anchor(text, clusters)` places a verse's clusters
  on the words of the verse's text (`ReferenceIndex`'s `VerseRef::text()`,
  notes left out — render skipped footnote words by counting), following
  the extension's `clusterAnchoring.ts`: word and parse clusters by their
  surface form, phrases by a run of words, the range choosing between
  repeats; what cannot land is reported with its reason.

Left for later, when a consumer needs them: `WordAnalyses.xml` and
`InterlinearSetup.xml` (not read), Unicode normalisation of forms (compared
lower-cased only), attaching glosses to tree nodes rather than to the
verse's plain text, and render's literal-translation pairing (a second
project verse by verse, which `ReferenceIndex` over each already allows).
Checked against render's real `Interlinear_ur_*.xml` only once Michael can
share one.

### Checked against a real file (2026-09-28)

Michael shared `Interlinear_ur_3JN.xml` from the ALEPH project (Sinaiticus,
Urdu glosses; 16 verses, 218 word clusters, no lexicon) with the book's
USFM. It is not committed. What it showed:

- Every range was exact: over an unedited verse, `Index` counts UTF-16 code
  units of `\v N ` followed by the verse's text, combining marks included
  (`αδελφω̅` is 7). Verse `1:0` is the text before `\v 1`: its one cluster
  glosses a word of the `\id` line, which `ReferenceIndex` has no verse for.
- The clusters are stored in no order; the reader and `anchor` already sort.
- 216 of the 217 verse clusters landed. The one that did not was the second
  of `ϋπερ γαρ γαρ` (1:7): the proportional prior chose the second `γαρ` for
  the first cluster. `anchor` now never takes a place a later cluster of the
  same form needs, and measures "nearest" by the offset most once-written
  words agree the ranges are shifted by (the marker, here), counted in
  UTF-16, falling back to proportion only when no word settles it. All 217
  land on the word their range names; with words written into 1:7 and 1:11
  since, they still land on the right ones.
