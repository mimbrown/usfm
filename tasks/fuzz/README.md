# Fuzzing

cargo-fuzz targets over the parser. Run on demand, not in the gate or CI:
libFuzzer needs nightly and a sanitizer, and a useful run takes minutes, not
seconds.

This crate is **not** a workspace member (the root `Cargo.toml` excludes it and
it carries its own `[workspace]` table), so `cargo build --workspace`,
`cargo clippy --workspace` and `scripts/gate.sh` never build it. Its own lints
are checked with `cd tasks/fuzz && cargo +nightly clippy --all-targets -- -D warnings`
(`rustup component add --toolchain nightly clippy` first: the nightly ticket 05
installs carries only `miri` and `rust-src`).

## Targets

`parse_lossy` and `parse_utf8` call `usfm_fuzz::check_source` (`src/lib.rs`),
which asserts what holds for *any* input, since the parser never fails:

1. parsing does not panic;
2. every span satisfies the invariants in `usfm_parser::span_check` — in
   bounds, not inverted, on character boundaries, and starting at the marker
   the node was read from (or, for a node the parser opened to recover, at or
   before the content it holds). `crates/usfm_parser/tests/spans.rs` asserts the same
   invariants over hand-written inputs, so the two cannot drift apart;
3. `to_usx_string` does not panic and produces well-formed XML, checked by
   reading it back with `xml-rs` to the end of the document.

`parse_html` calls `usfm_fuzz::check_html`, which parses the same way and then
asserts the same thing of the other writer: `to_html_string` does not panic,
and what it writes is well-formed markup. There is no HTML parser dependency —
`check_markup` is a 150-line scanner asserting that every start tag is closed
by the right name in the right order (`br`, `wbr`, `hr`, `img`, `meta`, `link`
and `input` close themselves, as does a `<… />` tag, and `<!-- … -->` is
skipped), that every attribute value is quoted with `"` and holds no raw `<`,
that outside a tag `<` only ever starts one and `&` only ever starts a
character reference, and that no character HTML cannot carry reaches the
output.

`roundtrip` calls `usfm_fuzz::check_roundtrip`, which parses, writes the tree
back as USFM with `usfm::codegen`, and parses *that*. The property is
idempotence on any input, not validity: whatever the recovery rules made of the
bytes, writing that tree out and reading it back must give the same thing.
Three assertions, the ones `usfm_tests::roundtrip` defines and explains
(the gate's `--roundtrip` step and `usfm_codegen`'s own test run the same
three):

1. the two trees are equal ignoring spans (`usfm_ast::eq_ignoring_spans`, which
   compares styles by marker rather than by index);
2. the second parse reports no diagnostic **code the first did not**. Gaining
   one means the writer wrote something the parser likes less than the input
   did. Losing one is the writer canonicalising a spelling the AST does not
   record, which is its job. It is deliberately not "the second parse reports
   no error": an error about the *document* rather than about its spelling — a
   `\v` with no `\c`, a book with no `\id` — is written back faithfully and
   reported again, and 21 of the 275 conformance cases do exactly that;
3. writing the second tree gives the same bytes, so a writer that produced
   something the parser reads differently cannot hide behind 1 and 2.

`read_usx` and `usx_roundtrip` (ticket 49, M7) are the USX reader's.
`read_usx` calls `usfm_fuzz::check_read_usx`: the bytes are USX, read through
`usfm::parse_usx` (the reader, then the semantic checks), and the reader, which
never fails either, must not panic, must keep every span in its source —
`usfm_parser::span_check::read_violations`, the invariants as they read for
XML, which `tasks/conformance/tests/usx_reader.rs` asserts over every
reference — and must build a tree `to_usx_string` writes as well-formed XML.
`usx_roundtrip` calls `usfm_fuzz::check_usx_roundtrip`: the bytes are USFM,
parsed, written as USX and read back, and

1. the read tree is the parse's, ignoring spans, after
   `usfm_usx::testing::normalise` on both — the list of what USX cannot say,
   which ticket 45 measured and this target added to, shared with
   `usx_reader.rs` rather than copied (the `testing` feature, as `span_check`
   is the parser's);
2. the read tree, written as USFM and parsed, passes `roundtrip`'s three
   assertions with the read as the first parse: the same tree, no code
   `parse_usx` did not report, a fixed point.

| Target | Input | Checks |
| --- | --- | --- |
| `parse_lossy` | any bytes, through `String::from_utf8_lossy` | parse, spans, USX |
| `parse_utf8` | any bytes, skipped unless they are valid UTF-8 | parse, spans, USX |
| `parse_html` | any bytes, through `String::from_utf8_lossy` | parse, HTML |
| `roundtrip` | any bytes, through `String::from_utf8_lossy` | parse, USFM, parse |
| `read_usx` | any bytes, through `String::from_utf8_lossy`, as USX | read, spans, USX |
| `usx_roundtrip` | any bytes, through `String::from_utf8_lossy` | parse, USX, read, USFM, parse |

## Running

```bash
# From the repository root. 10 minutes is the milestone's exit criterion.
cargo +nightly fuzz run --fuzz-dir tasks/fuzz parse_lossy -- -max_total_time=600 -max_len=65536
cargo +nightly fuzz run --fuzz-dir tasks/fuzz parse_utf8  -- -max_total_time=600 -max_len=65536
cargo +nightly fuzz run --fuzz-dir tasks/fuzz parse_html  -- -max_total_time=600 -max_len=65536
cargo +nightly fuzz run --fuzz-dir tasks/fuzz roundtrip   -- -max_total_time=600 -max_len=65536
cargo +nightly fuzz run --fuzz-dir tasks/fuzz read_usx    -- -max_total_time=600 -max_len=65536
cargo +nightly fuzz run --fuzz-dir tasks/fuzz usx_roundtrip -- -max_total_time=600 -max_len=65536

# Four workers, same wall time.
cargo +nightly fuzz run --fuzz-dir tasks/fuzz parse_lossy -- \
  -max_total_time=600 -max_len=65536 -workers=4 -jobs=4
```

`cargo install cargo-fuzz --locked` and `rustup toolchain install nightly` if
either is missing (`docs/agents/loop.md`).

A crash is written to `tasks/fuzz/artifacts/<target>/` (git-ignored) and the run
stops. Reproduce and minimise it:

```bash
cargo +nightly fuzz run  --fuzz-dir tasks/fuzz <target> tasks/fuzz/artifacts/<target>/crash-<hash>
cargo +nightly fuzz tmin --fuzz-dir tasks/fuzz <target> tasks/fuzz/artifacts/<target>/crash-<hash>
```

Then write the minimised input as a test — `crates/usfm_parser/tests/recovery.rs` for a
parser rule, `tests/spans.rs` for a span invariant, `tests/whitespace.rs` for a
whitespace rule, `tests/verse_ends.rs` for a verse-end placement rule,
`tests/usx_text.rs` for the
USX writer, `usfm_html`'s own tests for the HTML writer, `usfm_codegen`'s for
the USFM writer — and fix it. A finding
is a bug: it is not worked around by widening an invariant or catching the
panic. A finding that is not fixed in the same sitting goes to `findings/`
with a note — one input file and a section in `findings/README.md` saying what
it breaks and what fixing it would take — so it is not lost.

## Seeds

`./seed.sh` copies the conformance inputs into `corpus/parse_lossy/`,
`corpus/parse_utf8/`, `corpus/parse_html/`, `corpus/roundtrip/` and
`corpus/usx_roundtrip/` — one corpus per target, listed in the script's
`usfm_targets` array — named after the test they came from:

- the tcdocs inputs (`tcdocs/tests/*/*/origin.usfm`), as `<category>__<case>.usfm`,
  except `biblica/PublishingVersesNotClosed` and
  `biblica/PublishingVersesWithFormatting`, which quote the NIV (Biblica's
  copyright, not open data) and are left out by the script's `excluded` list;
- the vendored usfm-grammar fixtures
  (`tasks/conformance/fixtures/usfm-grammar/{autofix/*,bugfixes/*/origin.usfm}`), as
  `usfm-grammar__<dir>__<name>.usfm`. The `autofix` inputs are deliberately
  malformed, which is what makes them worth seeding.
- the vendored machine.py fixtures (`tasks/conformance/fixtures/machine-py/*/*.SFM`), as
  `machine-py__<project>__<book>.usfm`. Among them are a zero-byte book and
  two whose `\id` disagrees with their filename.
- the benchmark corpus's real aligned text (`tasks/benchmark/corpus/aligned/*.usfm`,
  ticket 10), as `usfm-js__<book>.usfm`: unfoldingWord's Acts, the English
  ULT in `\zaln-s`/`\zaln-e` and the Greek UGNT with `\w` attributes and
  `\k-s` key terms. Both are whole books, so both seeds are truncated. With
  them there were 297 seeds per target, where the runs recorded below started
  from 295; without the two NIV inputs there are 295 again.

`corpus/read_usx/` (the script's `usx_targets`) gets USX instead, 274 seeds:
the reference of every case above that has one (`<category>__<case>.xml` and
`usfm-grammar__bugfixes__<case>.xml`, the `.usfm` seed's name with `.xml`: 256
tcdocs references, the two NIV cases' left out with their inputs, and 13
usfm-grammar ones) and the vendored machine.py USX
(`fixtures/machine-py/usx/`, ticket 47) as `machine-py__usx__<project>__<book>.usx`
— the three WEB books and the malformed Tes pair. `WEB-DBL/1JN.usx` is
112 KiB, so its seed is truncated like any other and is no longer
well-formed XML; 2 and 3 John are the whole DBL shape. The references are CC
BY 4.0 and MIT with their inputs (`NOTICE.md`).

It is idempotent, and it truncates a seed longer than `-max_len` to the last
whole line that fits, which is what libFuzzer would do with it anyway. Run
`git submodule update --init tcdocs` first.

The seeds are committed. libFuzzer also *writes* to the corpus directory as it
finds new coverage, so after a run `corpus/` holds more than the seeds;
`./seed.sh --prune` removes everything that is not a seed again.

## Results

10 minutes per target on the final code (4 vCPUs, one worker each,
`-max_len=65536`), starting from the committed seeds. exec/s is low for a
fuzzer because the corpus carries whole books: one input is up to 64 KiB of
USFM, parsed and serialized. `parse_lossy` is the slower of the first two
because every input reaches the parser, while `parse_utf8` skips the ones that
are not UTF-8.

`parse_lossy` and `parse_utf8` were run side by side on `bfaa57f` (ticket 06);
`parse_html` on the ticket 14 tree, twice — the run below is the second, on the
final code, and the first (106 298 runs in 601 s, 177 exec/s, from the 295
seeds) was clean as well. Its corpus row is larger because the second run
started from what the first one had found; `./seed.sh --prune` puts the
committed corpus back to the seeds afterwards. `roundtrip`'s row is from its
twenty-third run, the first that was clean; each run before it started from the
pruned seeds too, so the depth in the table is what ten minutes reaches from
them.

| Target | exec/s | corpus at the end | cov | ft | crashes |
| --- | --- | --- | --- | --- | --- |
| `parse_lossy` | 49 (29 749 runs in 601 s) | 1647 files, 14.9 MB | 4175 | 22 337 | none |
| `parse_utf8` | 83 (50 097 runs in 601 s) | 1622 files, 13.7 MB | 4200 | 22 225 | none |
| `parse_html` | 166 (100 068 runs in 601 s) | 2110 files, 16 MB | 3408 | 20 830 | none |
| `roundtrip` | 94 (56 987 runs in 601 s) | 2256 files, 22 MB | 4636 | 23 380 | none |
| `roundtrip`, rerun on the ticket 49 parser | 115 (69 158 runs in 601 s) | 2483 files, 33 MB | 4770 | 24 105 | none |
| `read_usx` | 736 (442 912 runs in 601 s) | 2651 files, 25 MB | 6311 | 28 298 | none |
| `usx_roundtrip` | 40 (24 233 runs in 601 s) | 2158 files, 33 MB | 6923 | 32 048 | none |

The last three rows were run side by side on 2026-09-27, from the
pruned seeds (295 USFM, 274 USX), on the tree ticket 49 left; `roundtrip` was
rerun because the parser changed under it. `usx_roundtrip` is the slowest
target here by far: every input is parsed twice, written as USX, read twice
(the comparison needs its own copies, and `Document` has no `Clone`) and
written as USFM and parsed again.

`parse_html` found nothing in either run: the escaping it checks for went in
with the target (ticket 14), so the hole ticket 06 left in the HTML writer was
closed before the first run rather than by it.

`roundtrip` (ticket 27) was the opposite. The seed scan (`-runs=0`) failed on a
seed straight away — `41MATTes.SFM`, the verse end of ticket 28 — and each of
the twenty ten-minute runs after it found one more. Twenty-one findings,
nineteen bugs — two of the bugs were found
twice, by different inputs, and the second of those took two goes: the first
fix moved the blocks after parsing and left a verse end dangling, which is what
the second finding was. The runs got deeper as they went, because each started
from the corpus the one before had left: the first finding was 487 execs in,
the twentieth 34 553. The twenty-first, from the 295 committed seeds again
after `./seed.sh --prune`, found a twenty-second input at 35 101 execs — a
second spelling of the one finding ticket 27 left open, the `Block::Milestone`
the writer had no line to put on.

Ticket 35 closed that one (the rule is on `Block::Milestone`) and the
twenty-second run, from the pruned seeds, found one more at about 38 700
execs: `\v 1\vp\`, a published verse number holding a backslash, written raw.
The twenty-third is the clean one in the table. Twenty-three findings, twenty bugs in all, falling out as four in
`usfm_ast` (three in `add_child`, one in `NumberRange`'s display), fourteen in
the parser and two in `usfm_codegen`'s writers — **and every one of them is
fixed**, so `findings/` is gone. Recreate it only when a finding cannot be
fixed in the sitting it was found in; nothing is left there now.

Found on the way there, each fixed with the test named:

| Input | What was wrong | Test |
| --- | --- | --- |
| `\` | the implicit `\p` that holds content outside a paragraph has no marker in the source, so the span invariant as written did not describe it | `spans.rs::implicit_nodes_span_the_content_they_hold` |
| `\v\` | same, for a paragraph opened at a marker that is then dropped | same |
| `\periph T \v 2 b \v 3` | a `\v` on the title line leaves a synthesized space among the title's children, and the title's span was taken from the last text child, so it ended at offset 0 | `spans.rs::periph_title_span_ignores_synthesized_text` (parser fix) |
| `\periph\* n` | the title is a `Text`, so its span is the source run and its content is that run normalised; the checker expected the span to start at the title's first character | `spans.rs::periph_title_span_is_the_source_it_was_read_from` |
| `\0` | a C0 control character reached the USX output, which XML cannot carry at all | `usx_text.rs::characters_xml_forbids_are_replaced` |
| `\rb b\|"h=c"` | two bare values both became `gloss`, so the USX had one attribute twice | `recovery.rs::duplicate_attribute`, `usx_text.rs::repeated_attributes_are_written_once` |
| `\w a\|b<c="1"\w*` | an attribute name that is not an XML name went into the output verbatim | `recovery.rs::malformed_attribute_name`, `usx_text.rs::attributes_that_are_not_xml_names_are_dropped` |

And `roundtrip`'s twenty, all of them fixed (tickets 27 and 35). All but the
first are in `usfm_codegen`'s
`roundtrip.rs::the_fuzz_findings_round_trip`, which checks the property they
were found by; the first is a whole file, covered there by
`the_machine_py_fixtures_round_trip`. The rule each one broke has its own test
in the crate that was wrong.

| Input | What was wrong | Test |
| --- | --- | --- |
| `machine-py__Tes__41MATTes.usfm` (a seed) | a verse end was dropped when `\v` follows `\esbe` with no paragraph marker: it was handed to the `\esbe` line's own empty block list (ticket 28, parser fix) | `verse_ends.rs::a_verse_after_esbe_with_no_paragraph_marker_ends_the_one_before_the_sidebar`, `usx_text.rs::a_verse_after_esbe_closes_the_verse_before_the_sidebar` |
| `\ \* i` | a `\*` that ends no milestone leaves the whitespace on both sides of it, and the two text runs merged into a `Text` holding two spaces — which rule 1 says no `Text` holds (AST fix, in `add_child`) | `whitespace.rs::whitespace_collapses_across_a_dropped_marker` |
| `\p\* n` | the same drop with nothing to merge with: the whitespace stayed as the paragraph's first text's leading space, which rule 6 gives to the marker (AST fix) | `whitespace.rs::leading_whitespace_after_a_dropped_marker_is_not_text_either` |
| `\v 3\* x` | the same, one node along: `\v N` is always written with its space, so the text's own leading space made two (AST fix) | same |
| ` i\periph\v 2` | a `\v` on a `\periph` line opened a verse whose start was thrown away with the rest of the line while its end was emitted — into the paragraph before the periph (parser fix: peripheral matter closes no verse, the title line included) | `verse_ends.rs::a_verse_on_a_periph_line_opens_no_verse_end` |
| `\v 4-4t` | a verse range whose ends are the same number is written as that number — but the `t` on the end had nowhere else to go, so `4-4t` came back as `4`, a different verse (`usfm_ast` fix, in `NumberRange`'s `Display`) | `usfm_ast`'s `a_collapsed_range_keeps_an_end_modifier` |
| `\periph\|: `, `\periph\|s \` | a default attribute value that ends a `\periph` attribute list kept its trailing whitespace, and the writer's own line break read it back without (parser fix: the list ends with the line, so that whitespace is the line's) | `attributes.rs::a_periph_default_value_that_ends_the_list_drops_its_trailing_whitespace` |
| `\z\|"a=\*` | the separator the writer put between a default attribute and the pair after it was read back as part of the default's verbatim value (codegen fix: nothing is written after a default pair) | `usfm_codegen`'s `a_default_attribute_gets_no_separator_after_it` |
| `\e-\ef -\cat\0 \rb \5b\rb*\no"r*` | a note's `\cat` category is joined the same way as a `\periph` title, across a character style that contributes nothing, and doubled the whitespace at the seam the same way (parser fix, the same one) | `recovery.rs::note_category_collapses_whitespace_across_a_style` |
| `iT\n\periph\0*\n\w 1"\w*\np` | the `\periph` title is the text of its line, so a character style on it contributes nothing — and left the whitespace on both sides of itself side by side in the title (parser fix: the joined title collapses its whitespace, rule 1) | `recovery.rs::periph_title_collapses_whitespace_across_a_style` |
| `\v 1p\esb\esbe\.\v 7` | a verse after text on the `\esbe` line ended the one before it outside the sidebar, where the same document with a real `\p` after `\esbe` — which is what the writer writes — ends it inline after that text (parser fix: the `\esbe` line places verse ends as the implicit `\p` it becomes) | `verse_ends.rs::a_verse_after_text_on_the_esbe_line_ends_the_previous_one_inline` |
| `\esb\periph` | a `\periph` inside a sidebar swallowed the `\esbe` that closes the sidebar, as an empty paragraph styled `\esbe` — a sidebar no writer can close (parser fix: whatever ends the container a division is in ends the division) | `recovery.rs::a_periph_inside_a_sidebar_ends_with_the_sidebar` |
| `\periph\id\`, `\periph\id\v 3` | a block after a `\periph` whose `\id` the parser dropped: the division runs to the next `\periph` or `\id`, so the written form swallowed it — and a `\v` there opened a verse that a periph's blocks never do (parser fix: a dropped `\id` ends nothing, so the division simply continues, verses suspended) | `recovery.rs::a_block_after_a_periph_that_nothing_ended_is_inside_it` |
| `\tr \tc1 x\c\n\tr \tc1 y` | two tables with nothing between them but a dropped `\c`: consecutive `\tr` rows are one table, so the written form ran them together (parser fix: so does the parser now) | `recovery.rs::two_tables_with_a_dropped_marker_between_them_are_one` |
| `\c 3\c\n\cp` | a `\cp` paragraph after a chapter start, which only a dropped marker can put there: `\c` absorbs a `\cp` that follows it, so the written form read it back as the chapter's published number (parser fix: so does the parser now) | `recovery.rs::a_cp_paragraph_after_a_chapter_is_its_published_number` |
| `r\id\-\*` | a block-level milestone whose paragraph was closed by an `\id` the parser then dropped: nothing stood between them in the tree, and written out the paragraph ran on and swallowed the milestone (parser fix: such a milestone goes inside the paragraph) | `recovery.rs::a_block_milestone_after_a_paragraph_that_nothing_closed_is_inline` |
| `\v 1\v\vp` | a `\va` or `\vp` that reached a paragraph as a character style, because the `\v` between it and the verse was dropped: a writer has nowhere to put `alt_number` and `pub_number` but right after the number, so the written form read them back as those (parser fix: so does the parser now, wherever they come from) | `recovery.rs::a_va_or_vp_after_a_verse_is_its_number_however_it_got_there` |
| `\v 1\vp x` | an unclosed `\vp` after `\v N` stayed beside the verse as a `Char`, a tree no USFM spells: closing it, as the writer must, reads it back as the published number (parser fix: it is lifted whether or not it is closed) | `recovery.rs::published_verse_number_not_closed` |
| `\esb\c\sh\*`, `\p x\n\tr \tc1 y\n\c\n\zaln-s\*`, `\periph\id\e\*` | a `Block::Milestone` the writer has no line to put on: `\esbe`, a `\tr` row and a `\periph` title all run to the next paragraph marker, so the written form takes the milestone back — and the `\periph` title, which keeps only text, dropped it without a diagnostic (ticket 35, parser fix: such a milestone opens an implicit `\p`, the rule is on `Block::Milestone`) | `recovery.rs::a_block_milestone_after_a_sidebar_is_inside_an_implicit_paragraph`, `…_after_a_table_…`, `…_at_the_head_of_a_periph_…`, `usx_text.rs::a_milestone_on_a_periph_title_line_reaches_the_output` |
| `\v 1\vp\` | a published verse number holding a backslash, written raw: it and the `\` of the `\vp*` after it made one escape, the closing marker was gone and the number came back as `\vp*` (ticket 35, codegen fix: `\vp`'s number is written as text; `\cp`'s stays verbatim, and the parser now folds a `\cp` paragraph only when a `Word` token could carry its first word) | `usfm_codegen`'s `a_published_number_is_written_the_way_its_marker_reads_it` |

### The USX reader's targets (ticket 49)

Before the clean runs above came two ten-minute discovery rounds in fork mode
(`-fork=2 -ignore_crashes=1`, so one run collects every finding instead of
stopping at the first), with the seed scan first. The seed scan failed at
once on `usx_roundtrip`: seven `fail` cases of tcdocs and usfm-grammar, which
the harness never compares, read back to a tree other than their parse. The
first round then gave 37 `read_usx` crashes, all one finding, and 132
`usx_roundtrip` ones, which triage (the parse and the read tree side by side,
`tmin` on the smallest of each kind) sorted into the rows below. The second
round, on the fixed code, found nothing in either target, nor in `roundtrip`
run beside them. Seven bugs — four in the reader, two in the parser, one in
the USX writer — and three entries the list of what USX cannot say lacked;
each has the test named, and the bugs were each a failing test before the fix.

| Input | What was wrong | Test |
| --- | --- | --- |
| `<char style="w" x\u{fffd}-morph="a">` (every `read_usx` crash) | the span check for a read tree took an attribute's span to start with a *valid* USFM name; XML allows more (U+FFFD is a name character), the reader keeps the name as the parser would, for `malformed-attribute-name`. The checker was wrong: it now asks for the name as written | `usx_reader.rs::a_malformed_attribute_name_keeps_its_span` |
| `paratextTests/UnmatchedSidebarEnd` (a seed) | `<para style="esbe">` read silently, where the parser reports `\esbe` with no `\esb`: the USFM of the read tree reported a code the read had not (reader fix) | `reader.rs::a_sidebar_end_paragraph_is_an_unmatched_sidebar_end` |
| `\v 4\ip\w p\v 4` | a verse that starts inside a character style of a paragraph that is not verse text had its end put in the paragraph *before*, ahead of its own start: "the paragraph a verse started in" looked only at the paragraph's own children (parser and reader fix, the rule is mirrored) | `verse_ends.rs::a_verse_that_starts_inside_a_character_style_ends_in_that_paragraph`, `usx_reader.rs::a_usx_2_file_is_the_parse_of_its_usfm` |
| `\v 8\p\v 0\w\v 1` | two verse ends waiting at once — one for block level, one for the head of a character style — and the second replaced the first, so verse 8 never ended (parser and reader fix: a stack, not one slot) | `verse_ends.rs::an_end_waiting_for_block_level_is_kept_when_a_second_one_waits`, `usx_reader.rs::a_usx_2_file_is_the_parse_of_its_usfm` |
| `\thc0`, `\tc3-2` in a table | the reader refused a cell style the parser reads (column 0, a span that ends before it starts) and fell back to `tc1` (reader fix) | `read.rs`'s `a_cell_style_is_header_alignment_and_columns` |
| `\w\|.\t` | a tab in an attribute value was written raw, and an XML reader turns it into a space; `xml-rs` escapes a line feed and a carriage return but not a tab (writer fix: the writer escapes attribute values itself) | `xml_document.rs`'s `whitespace_in_attribute_values_survives_a_reader` |
| `\fe\xta\xt\ft\xt.\xt*` | the nesting scan counted the `\+xt*` ahead, which closes a `\+xt`, as an unclosed `\xt`'s own closer, so the writer's `\xta \xt \ft \+xt .\+xt*` read `\xt` into `\xta`. `roundtrip` fails on this input too; it came after that target's last clean run (parser fix) | `recovery.rs::a_plussed_style_ahead_claims_its_own_closer`, `usfm_codegen`'s `the_fuzz_findings_round_trip` |
| `\id MRK …` then block `\zaln-e\*`, and `\usfm 3.1.test . \v 1 …` | `\usfm` ran on like any paragraph, taking in a milestone on the next line and a verse on its own, and the USX writer, which writes that paragraph as `<usx version>`, dropped them; a USX file with an `<ms>` after `<book>` read to a tree no USFM spelled (parser fix: `\usfm` is read like `\id`, its text and nothing else, and a block milestone may follow it) | `recovery.rs::the_usfm_line_holds_only_its_version` |

And three entries for `usfm_usx::testing::normalise`, each something the
writer drops or replaces by design, not a bug: an attribute USX cannot carry
(a bare value where the marker has no default attribute, a malformed name, a
repeat, `style`), with the empty `|` list the spec expected and ticket 45
had not measured (`paratextTests/EmptyFigure` and five other seeds); a
character XML forbids, which the writer has written as U+FFFD since ticket 06
(most of the first round's crashes); and where a `\usfm` stood and every one
after the first, since `<usx version>` is all USX has of it.
