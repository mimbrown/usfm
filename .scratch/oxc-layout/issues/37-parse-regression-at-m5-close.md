# 37. `parse` is 3.9% slower at the M5 close than at the M4 close

Status: ready-for-agent
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
