# 37. `parse` is 3.9% slower at the M5 close than at the M4 close

Status: resolved
Milestone: M5

Found at the M5 boundary by the rerun `docs/agents/loop.md` prescribes
(`docs/benchmarks.md`, "M5 close"): against `f181eab` (the M4 close), built
the same way and run turn about three rounds each with codegen units pinned,
`parse/whole-corpus` is **−3.9%** (52.3 -> 50.2 MiB/s, medians), which is
over the spec's 3% threshold; `parse_semantic` is −3.4% and `parse_html`
−3.1%, both riding on the parser; `lex`, `parse_usx`, `reference_index` are
within noise and `analyze` is +3.5%. The M5-close spread on `parse` was 2.9%
against 0.3% on the baseline, so the first job is to confirm the number.

Nothing in M5 was meant to touch the parse path; what did touch it are the
round-trip fixes of tickets 27, 35 and 36, all of them per-node work in hot
places:

- `usfm_ast::InlineContainer::add_child` now checks the last child and trims
  every `Text` added first or after a `VerseStart` (rule 6), and the merge
  path checks `ends_with` whitespace — one branch per inline node, every
  node;
- `parse_char`'s close path calls `fold_verse_number_style`, which resolves
  `marker_name` (a string compare against `"va"`/`"vp"`) for every character
  style that closes;
- `place_block_milestone`, `parse_paragraph_as`'s `text_type_of` lookup, the
  `\cp` fold check (`marker_name(marker) == "cp"` on every paragraph close),
  `parent_holds_plain_text` (only on a nesting candidate — rare);
- `collapse_ascii_whitespace` on periph titles and note categories (rare).

- Confirm with the recipe in `docs/benchmarks.md` ("Reading a regression":
  both binaries built with `CARGO_PROFILE_BENCH_CODEGEN_UNITS=1`, run turn
  about, median of three). If it does not reproduce over 3%, record that and
  close.
- If it does: attribute it the way ticket 24 did — time `parse` with each
  suspect switched off, or callgrind — and fix what is found without
  changing any tree or diagnostic (the round-trip step and the snapshot
  suites are the check). Likely cheap wins: compare `StyleId`s cached at
  parser construction (`self.va`, `self.vp`, `self.cp`, as `self.p` and
  `self.esbe` already are) instead of `marker_name` string compares; make
  the `add_child` rule-6 check a fast path that only trims when the text
  actually starts with ASCII whitespace.
- Record the result in `docs/benchmarks.md` ("After ticket 37") with the
  same method, and update the M5 close note in the spec.

Done when `parse/whole-corpus` is within 3% of the M4 close by the recipe
(or the regression is shown not to reproduce), the gate is green, and no
diagnostic or tree changed.

## Answer

Landed via PR #37 (2026-09-20). Confirmed first: `corpus-m4-close` against
`corpus-m5-close`, three rounds turn about on `parse/whole-corpus`, medians
53.7 vs 51.5 MiB/s, −4.1%. Attributed by callgrind instruction counts over
one parse of the whole corpus (wall clock cannot resolve suspects worth
0.2–2% each on this VM): M5 was +51.4 M instructions (+3.4%) on M4, of
which `fold_verse_number_style`'s `marker_name` call — an owned `String`
per closed character style — was 51%, the `\cp` `marker_name` compare per
paragraph close 17%, `add_child`'s rule-6 `last()` read on every inline
node 23%, and the merge path's `ends_with` under 1%; `parent_holds_plain_text`,
`place_block_milestone` and the rest were free. The rows add to the
measured loss.

Fix: `ParserImpl` caches the `va`, `vp` and `cp` indices at construction,
as it did `p`/`esb`/`esbe`/`c`/`tr`/`cat`/`periph`, and the two paths
compare ids; `add_child` asks the child first (a `Text` whose first byte is
ASCII whitespace) before reading the child list, and the merge check is a
last-byte test. No check removed, no tree, diagnostic or snapshot changed.
After: 1,504 M instructions (+0.7% on M4), and by the wall-clock recipe
`parse/whole-corpus` is **+1.4%** on the M4 close (51.8 -> 52.5, medians of
three, same sitting), `parse_semantic` −0.4%, `parse_html` +0.7%, `lex`
−2.0%. Recorded in `docs/benchmarks.md` ("After ticket 37"). Rule for the
future, now in CLAUDE.md: a hot path compares a cached index, never
`marker_name`. Ticket 30 is unblocked.
