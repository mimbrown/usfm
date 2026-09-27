# 56. Paratext interlinear glosses and lexicon, as aligned USFM?

Status: ready-for-human
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
