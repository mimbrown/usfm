# usfm-tools

Rust-based USFM parser with AST, language server, and multiple output formats.

## What "Progress" Means

This project has three parallel work streams, all in progress:

1. **Parser Development** (`crates/usfm_parser/`, `crates/usfm_ast/`)
   - Parse USFM into AST structure
   - Handle edge cases and malformed input gracefully
   - Expand grammar coverage

2. **Language Server** (`wip/usfm_language_server/`) — parked until M6
   - Power VS Code extension for editing USFM
   - Validation, diagnostics, formatting
   - Eventually: completion, hover, go-to-definition
   - Parked outside the workspace in `wip/` (ticket 01) and does not build;
     M6 (tickets 30–33, ticketed 2026-09-20) rebuilds it in `apps/` on
     `ParseResult`, `usfm_semantic` and `usfm_codegen`, and ticket 33 then
     deletes the parked copy. Work this stream through those tickets only.

3. **Output Generation** (`crates/usfm_style/`, future crates)
   - Transform AST to HTML, Dart, XML, etc.
   - Support web pages, PDFs, mobile apps

Progress on any of these fronts is valid. When asked to "make progress":
- Check for failing tests and fix them
- Implement missing AST node types
- Improve error recovery in parser
- Add output format support
- Enhance language server features

## Current Focus

**Parser hardening.** See `docs/plans/hardening.md` for the phased plan and
design decisions. The parser never fails: it recovers and reports
`Diagnostic`s in a `ParseResult`; `ParseResult::strict()` is the policy for
pipelines. Those four types (`Diagnostic`, `Code`, `Severity`, `ParseResult`,
plus `Diagnostic::render` / `to_json_line`) live in the `usfm_diagnostics`
crate, re-exported as `usfm_parser::diagnostics` (ticket 12) and as
`usfm::diagnostics` on the facade (ticket 17). Every recovery
rule is a `Code` variant with a test in `crates/usfm_parser/tests/recovery.rs`;
every *semantic* rule — a check that reads the finished tree and repairs
nothing, so `Code::is_semantic()` is true and `usfm_semantic::analyze` emits it
— is a `Code` variant with a test in `crates/usfm_semantic/tests/checks.rs`
(ticket 19). Between them the two files cover every `Code`, and each one's
coverage test reads `is_semantic()` to know which half is its own. Only
`usfm::parse` / `parse_with` / `parse_with_options` return both halves;
`usfm_parser::Parser::parse` returns the parser's alone.

Conformance status (276 tests across two roots, 2026-09-19):
- tcdocs (260 tests): 215 passed, 0 failed, 0 panicked, 1 skipped,
  44 expected failures, 0 unexpected passes
- `usfm-grammar/bugfixes` (16 tests, vendored under
  `tasks/conformance/fixtures/usfm-grammar/`, MIT): 16 passed, 0 failed
- `tasks/conformance/fixtures/machine-py/` (sillsdev/machine.py, MIT) is *not* a harness
  root: those Paratext-shaped projects ship no reference USX, so their
  expectation is the parser's own tree and diagnostics, snapshotted by the
  `machine_py_*` tests in `crates/usfm_parser/tests/recovery.rs`. All seven books are
  fuzz seeds.
- `BookCode::Other([u8; 3])` holds a code USX accepts that the enum does not
  name: `book@code` in `usx.rnc` is the book list *or* the pattern
  `[A-Z][A-Z0-9]{2}|[0-9][A-Z][0-9]|[0-9]{2}[A-Z]`, so `\id TST` is valid and
  round-trips as `<book code="TST">`. `unknown-book-code` (Error, `\id`
  dropped) now means a code that matches neither; the new
  `unlisted-book-code` (Warning, nothing dropped) means one that matches the
  pattern but no listed book.
- Harness semantics: `pass` inputs must match USX with no error diagnostics;
  `fail` inputs must either report an error or match the expected USX; a
  `pass` input with no `origin.xml` at all (three usfm-grammar cases) must
  parse with no error diagnostics, which is then the whole assertion; a
  `fail` input with no `origin.xml` and no error reported is skipped.
  The usfm-grammar reference files are compared with whitespace inside text
  collapsed on both sides and their `<usx version>` restored from the `\usfm`
  line, because that generator copies source whitespace verbatim and truncates
  the version to `major.minor`; tcdocs is compared exactly as before.
- Fourteen reference files (nine tcdocs, five usfm-grammar) are read through a
  patch in `tasks/conformance/tcdocs-patches/`
  (rules in its README; the directory covers both roots): a unified diff with
  the rationale above it, for a reference quirk or an accepted deviation, never
  for a parser gap. The harness fails a patch that stops applying or that the
  parser no longer needs. `cargo run -p usfm_tests -- --show <name>` prints one
  test's diagnostics, output and patched expected USX.
- `crates/usfm_parser/usfm-extra.sty` is appended to Paratext's `usfm.sty` by
  `crates/usfm_parser/build.rs`: the markers that sheet predates (`\ipc`, `\ta`,
  `\wl`) and the one entry it gets wrong (`\xta` occurs under `\ex` too),
  each citing `tcdocs/grammar/usx.rnc`.
- AST snapshot corpus: `cargo test -p usfm_parser --test snapshot` (review with `cargo insta review`)
- Run `git submodule update --init tcdocs` first. Without it the `usfm_tests` build
  fails on purpose, so a zero-test run can never report a pass rate.
- CI (`.github/workflows/ci.yml`) runs the unit and integration suites
  (`recovery`, `snapshot`, `spans`, `whitespace`, `attributes`, `verse_ends`, `usx_text`,
  `usfm_html`'s `footnotes`, `usfm_json`'s `json` and `coverage`,
  `usfm_semantic`'s `checks`, `usfm_codegen`'s lib and `roundtrip`, parser lib),
  gates lint with
  `cargo clippy --workspace --all-targets -- -D warnings` (in `scripts/gate.sh`
  since 2026-09-19, ticket 02: the workspace is clippy-clean, so a new warning
  fails the build) and
  gates tcdocs on `tasks/conformance/tcdocs-baseline.txt`, the list of known failures
  (currently empty). It fails on a regression *and* on a stale entry, so when
  you fix a tcdocs case, remove it from the baseline (or regenerate with
  `--write-baseline`) in the same commit. After it (ticket 27) comes
  `cargo run -p usfm_tests -- --roundtrip tasks/conformance/roundtrip-known.txt`:
  the round trip (parse -> USFM -> parse) over every conformance case of every
  root, `pass` and `fail` alike, about a second. `roundtrip-known.txt` is
  empty, has the baseline's semantics — an unlisted failure is a regression, a
  listed case that round-trips is a stale entry, both fail — and every entry
  needs a reason naming the bug. Last in the gate is `scripts/miri.sh`
  (ticket 05): `cargo +nightly miri test` over the `usfm_span`, `usfm_ast` and
  `usfm_diagnostics` libs, `usfm_usx`'s lib, `usfm_html`'s `escape` tests, the
  parser lib
  and the `whitespace`, `attributes`, `usx_text`, `verse_ends`, `spans` and
  (10 of 85) `recovery` suites, about 3 min 50 s. It needs a nightly toolchain
  with `miri` and `rust-src`, which CI installs in its own step and
  `scripts/session-start.sh` installs on a fresh VM; `rust-toolchain.toml`
  stays pinned at 1.98.0 for everything else.
- Fuzzing is on demand, not in the gate or CI (ticket 06): `cargo +nightly fuzz
  run --fuzz-dir tasks/fuzz parse_lossy -- -max_total_time=600 -max_len=65536`,
  and the same for `parse_utf8`, `parse_html` and `roundtrip`. The first two
  assert no panic,
  the span invariants (`usfm_parser::span_check`, shared with `crates/usfm_parser/tests/spans.rs`
  behind the `testing` feature) and that the USX output is well-formed XML;
  `parse_html` (ticket 14) asserts no panic and that the HTML output is balanced
  and properly escaped, checked by a scanner in `tasks/fuzz/src/lib.rs` rather
  than by an HTML parser; `roundtrip` (ticket 27) parses, writes USFM and parses
  again, asserting the same three things the gate's `--roundtrip` step does —
  the tree is equal ignoring spans, the second parse gains no diagnostic code,
  and the output is a fixed point. `tasks/fuzz` is
  outside the workspace, so run its clippy separately; see `tasks/fuzz/README.md`.

**Traversal API (Phase 3, complete 2026-09-12)** lives in `usfm_ast`:
`visit::Visit` / `visit_mut::VisitMut` (generated from one macro, same method
names, no context parameter: hold `document.style_sheet()` if you resolve
styles), `fold::Fold` (bottom-up, one associated type per node kind), and
`text::PlainText`. The parser re-exports the visitor modules. `Cursor` stays
read-only. `ReferenceIndex` (chapters, verses, their milestone paths,
`verse(c, v)`, `VerseRef::nodes()` / `text()`) left for `usfm_semantic` with
ticket 22 — it is derived from the tree, not part of it, and the AST freeze is
about node shape, not this helper. Build it with
`usfm_semantic::ReferenceIndex::new(&document)` (`usfm::ReferenceIndex` on the
facade); `Document::reference_index()` is gone. Chapters and verses come in
document order, one entry per `\c` / `\v`, repeats and all; `chapter(n)` and
`verse(c, v)` answer with the first match, and a `\v` before the first `\c`
has `chapter() == None` and is in no chapter's `verses()`.

Recent progress:
- **The M5 parse regression, paid back (ticket 37).** `parse/whole-corpus`
  was −3.9% at the M5 boundary and is **+1.4%** on the M4 close now, with no
  tree, diagnostic or snapshot changed and no check removed. Attributed by
  callgrind rather than by wall clock, because each suspect is worth under
  this VM's noise floor (`docs/benchmarks.md`, "After ticket 37", has the
  table and the method). Two thirds of the loss was `marker_name` — which
  returns an **owned `String`** and is meant for diagnostic paths — called
  per closed character style (`\va`/`\vp`) and per closed paragraph (`\cp`);
  `ParserImpl` now caches those three `StyleId`s at construction, as it did
  `p`, `esb`, `esbe`, `c`, `tr`, `cat` and `periph`, and compares ids. The
  other quarter was `add_child`'s rule-6 check reading the child list on
  every inline node; it asks the cheap question first now (is this a `Text`
  whose first byte is ASCII whitespace), and the merge path's `ends_with` is
  a last-byte test. **When a hot path needs to know which marker it has,
  compare a cached index — never `marker_name`.**
- **Where a `Block::Milestone` may stand (ticket 35, M5).** The rule is on
  `Block::Milestone`: one stands between blocks only when the block before it
  is one the writer gives a line of its own — a `Book`, a `ChapterStart`, a
  `ChapterEnd`, another block milestone, or nothing at the head of the
  *document's* block list. After a `Para`, a `Table`, a `Sidebar` or a
  `Periph`, and at the head of a sidebar's or a periph's own list, it belongs
  to an implicit `\p`, because `\p`, `\esbe`, a `\tr` row and a `\periph`
  title all run to the next *paragraph* marker and take back a milestone
  written on the line after them. USFM has no marker that ends a line, so
  there is nothing else to write. `place_block_milestone` implements it,
  `parse_blocks` carries a `BlockListHead` for the head case, and no reference
  file disagrees: the only block-level `<ms>` in either conformance root
  (`usfmjsTests/ts`, `ts_2`) follows a `<chapter>`. With it, the `\periph`
  title line stopped swallowing things: only its text is the title, and
  whatever else stood on it — a milestone, a character style, a note, a `\v` —
  opens the division's implicit `\p` instead of vanishing (a hole in D1), so
  `\periph T` + `\qt-s\*` and `\periph T` + `\p \qt-s\*` are one tree. Also in
  the ticket, from the fuzz run that followed: `\vp`'s published number is
  written as text (a `\` in it ran into the `\vp*` after it), while `\cp`'s
  stays verbatim — `\cp` reads one `Word` token, so the `\cp`-paragraph fold
  of ticket 27 now gives up a first word only when a `Word` token could be it.
- **The round trip as an invariant (ticket 27, M5).** parse -> USFM -> parse is
  now asserted in three places off one definition
  (`usfm_tests::roundtrip::check`): the trees are equal ignoring spans, the
  second parse gains no diagnostic **code the first did not**, and writing the
  second tree gives the same bytes. It is deliberately not "the second parse
  reports no error" — measured: 21 of the 275 conformance cases keep one, and
  each is a document still wrong after being written out faithfully
  (`missing-id`, `verse-outside-chapter`, `empty-book`, …), which a writer
  could only "fix" by editing the document. The three places are
  `crates/usfm_codegen/tests/roundtrip.rs` (the `pass` corpus, the 86 benchmark
  books and now the seven machine.py fixtures), the gate's
  `--roundtrip tasks/conformance/roundtrip-known.txt` step over every
  conformance case of both roots, and `tasks/fuzz`'s `roundtrip` target.
  **Twenty-one bugs came out of it** — twenty the `roundtrip` target found,
  over twenty-three findings (one of them on the seed scan, which is ticket
  28), and ticket 29, which the ticket named. **All of them are fixed**, the
  last two by ticket 35, and `roundtrip` then ran ten minutes clean from the
  seeds on its twenty-third run (56 987 runs, 94 exec/s), so
  `tasks/fuzz/findings/` is gone and every target has had a clean run.
  Each one became a test first and then a fix in the crate that was
  wrong; the full table, input by input, is in `tasks/fuzz/README.md`, and
  every input is in
  `usfm_codegen`'s `roundtrip.rs::the_fuzz_findings_round_trip`. They are
  almost all one shape: **a marker the parser drops leaves a tree no USFM
  spells**, because what the writer writes to stand in its place is read back
  differently. So the parser now keeps the document writable —
  - whitespace a dropped marker left behind obeys rules 1 and 6 again
    (`usfm_ast`'s `InlineContainer::add_child`, the one place runs merge and
    children are added: `\ \* i` had made a `Text` of two spaces, `\p\* n`
    and `\v 3\* x` one with a leading space), and so does text joined across
    a child that contributes none (`parse_periph`'s title and a note's `\cat`,
    both through `collapse_ascii_whitespace`);
  - a node that the construct before it would absorb goes into that construct
    instead of standing beside it: a `Block::Milestone` after a `Block::Para`,
    a `\cp` paragraph after a `ChapterStart`, a `\va`/`\vp` `Char` after a
    `VerseStart` (they are the verse's numbers wherever they came from), an
    unclosed `\vp` after `\v N`
    (now lifted into `pub_number` closed or not), a second `Table` after a
    `Table` (consecutive `\tr` rows are one table), a block after a `Periph`
    (a `\periph` division runs to the next `\periph` or `\id`, and an `\id`
    the parser drops is not one, so the division simply carries on — and
    whatever ends the container a division is in ends the division, or a
    `\periph` in a sidebar swallows the `\esbe` that closes it);
  - `\periph` lines close no verse (a `\v` there emitted an end into the
    paragraph before the periph while its own start was thrown away with the
    rest of the line; verses are suspended across a periph now, and ticket 35
    keeps the start), and a default attribute value that
    ends one drops its trailing whitespace, which the line break the writer
    ends that line with eats anyway;
  - two writers were wrong rather than the parser: `usfm_codegen` wrote a
    separator after a default attribute that the parser read back into the
    value, and `NumberRange`'s `Display` dropped an end modifier on a range
    whose ends are the same number (`\v 4-4t` came back as `\v 4`).
  Ticket 28's own fix is in `parse_sidebar` (the sidebar is pushed before the
  `\esbe` line is parsed, so the verse end lands before it), with the `\esbe`
  line now placing verse ends as the implicit `\p` it becomes; ticket 29's is
  `eat_whitespace()` before the pipe on the block milestone path, after which
  `usfm_codegen` writes `\qt-s |who="…"\*` with the space USFM 3 spells.
- **`usfm format` and `--format usfm` (ticket 26, M5).** `usfm format <files>`
  parses each file on its own through the facade and writes it back with
  `usfm_codegen`: to stdout by default, `--write` in place, `--check` for CI
  (exit 1 and `would reformat <path>` per file that differs, nothing written).
  `--write` refuses a file whose parse reported an Error unless `--force`, since
  the repaired tree is not the author's text; `--write` and `--check` together
  is a usage error. `--stylesheet` and `--diagnostics text|json` are `parse`'s.
  The driver is `apps/usfm_cli/src/format.rs` — no `Driver`, because a
  formatter must keep the files apart where `parse` concatenates them — sharing
  `read_source`, `read_stylesheet` and the new `print_diagnostics` with
  `driver.rs`. `usfm_pipeline::OutputFormat::Usfm` is the same writer behind
  `usfm parse --format usfm` (no diglot form), and the facade's `pipeline`
  feature pulls in `codegen`. **`format --check` exits 0 over all 86 books of
  `tasks/benchmark/corpus/web/`** since ticket 34, which moved the writer to
  the two spellings the corpus (and every other USFM writer) uses; when this
  ticket closed it reported 78 of them, and the criterion recorded here was the
  weaker one — after a `format --write` pass, `--check` exits 0 with no
  diagnostic. Both hold now, the second because the first does.
  `apps/usfm_cli/tests/cli.rs`'s `check_passes_on_the_whole_benchmark_corpus`
  guards it (one process, under a second)
- **Idiomatic notes, and `\cp` on its own line (ticket 34, M5).** Two writer
  spellings changed, both in `crates/usfm_codegen/src/usfm.rs`, neither of them
  a tree change. A note's content runs are no longer closed:
  `\f + \fr 1.1 \ft the note\f*`, not `\ft the note\ft*\f*`. The styles this
  applies to are the stylesheet's, not a list in the crate — a
  `\StyleType Character` entry whose `\TextType` is `NoteText`, which is `\fr`,
  `\ft`, `\fq`, `\xo`, `\xt`, `\cat` and eighteen more — and the closer goes
  only when what follows is another of them or the note's own closer, since
  anything else (text, a milestone, a style from outside the note vocabulary)
  would be read as content. Two more keep it: an attribute list runs to the
  closing marker, and `\xt` is the only note-internal marker with `NEST`, so a
  sibling in front of a *closed* `\xt` keeps its own closer or the `\xt`
  becomes its child (`omitted_closers` decides the note's children from the
  right for exactly that reason; since ticket 36 that clause is one case wider
  than the parser needs, because an `\xo` takes no un-plussed child at all, and
  the writer keeps the `\xo*` anyway — no corpus writes the shape and `\xo*`
  reads back to the same tree). The parser reports nothing for an implicit
  close inside a note — `parse_char_body` is silent when `note_depth > 0` —
  so the second parse gains no code and the round trip holds unchanged.
  `\ca` and `\cp` now go on lines of their own after `\c N`, which is what the
  spec's own example (`tcdocs/tests/specExamples/chapter-verse`) and every
  corpus in the repo spell; `\c` already looked past the line break for both
- **`usfm_codegen`: USFM from the AST (ticket 25, M5).** `to_usfm_string(&Document)`
  (and `write_usfm` into any `fmt::Write`), a `Visit` writing into a `String`,
  re-exported as `usfm::codegen` behind the default-on `codegen` feature. M5's
  exit criterion holds: **every one of the 223 `pass` cases of the two
  conformance roots and all 86 books of the benchmark corpus** parse, write and
  parse again to the same tree, with an empty `KNOWN` list
  (`crates/usfm_codegen/tests/roundtrip.rs`). The output is *not* the source
  byte for byte and is not meant to be — the AST records what a construct is,
  not which spelling the source used, so the writer emits one canonical
  spelling: an implicitly closed `\add` comes back with its `\add*`, a `\nd`
  nested without `+` comes back with one, `\v 01` as `\v 1`, an unquoted
  attribute value in quotes, a stray `\` as `\\`. So the round-trip test asks
  that the second parse gains no diagnostic (five cases lose
  `character-style-nested-without-plus`, which is the canonicalisation) and
  that writing the second tree gives the same text back — a fixed point. Two
  spellings are forced rather than chosen: a milestone's `|` is flush against
  its marker (`\qt-s|who="God"\*`), because between blocks the parser looks for
  it with no whitespace eaten first, and `\cp` takes no closing marker where
  `\vp` does. Equality is `usfm_ast::eq_ignoring_spans`, which also compares
  styles by **marker name through each document's own sheet**, since a derived
  style's `StyleId` is an index into the sheet that derived it
- **The semantic pass, made cheap (ticket 24, M4).** `parse_semantic` ran
  11–14% behind `parse` over the whole corpus; it is now 7–8%, and what is left
  is the cost of walking a built tree a second time rather than of any check —
  a callgrind profile puts every cache miss in the walk and none in the checks,
  so the 5% the ticket asked for is under the floor a second traversal can
  reach. Nothing moved back into the parser and no diagnostic, message or span
  changed. Three things paid for it, found by timing `analyze` alone (a new
  bench group) with each check family switched off:
  `usfm_ast::is_valid_attribute_name` scans bytes instead of `chars()` and is
  `#[inline]` — decoding UTF-8 to classify an attribute name was 58% of
  everything the pass did over an aligned text; `check_placement` remembers its
  verdict per (style, parent) in a 64-slot direct-mapped memo, where it scanned
  `\w`'s 96-entry `OccursUnder` list for every word; and `check_verse_order`
  has a fast path for a plain `\v 5`. The scope stack is four fields saved on
  the Rust stack instead of a `Vec` of frames, so `placement_parent` is a field
  read rather than two scans. Numbers, attribution and method are in
  `docs/benchmarks.md` ("After ticket 24")
- **Verse and chapter order (ticket 23, M4).** Four Warnings, all
  `usfm_semantic`'s, all riding the `Analyzer` walk (no `ReferenceIndex`: the
  checks want a chapter number, a verse number and a span, and building an
  index for them cost 9 points of `parse_semantic`):
  `duplicate-verse-number` and `verse-out-of-order` over each chapter's verses,
  `duplicate-chapter-number` and `chapter-out-of-order` over each book's `\c`
  markers — **per book**, because a second `\id` starts a book whose chapters
  number from 1 again and the CLI makes such a document out of several files.
  Coverage is by number *with* its segment, so
  `\v 4a` and `\v 4b` are two verses while a bare `\v 4` is the whole of verse
  4 and collides with either; a range covers its endpoints as written and
  everything between them unsegmented (`\v 3-4a` then `\v 4b` is fine, `\v 3-5`
  then `\v 4` is a duplicate). A verse reports at most one of the two codes,
  the duplicate first; a verse before the first `\c` of its book is skipped
  (`verse-outside-chapter` has already said it). Versification stays out of
  scope: nothing here reports a *missing* verse. Reported on the later verse's
  `VerseStart` or the later `ChapterStart`. 22 codes are semantic now. tcdocs
  is unchanged at 231 / 0 / 44 (Warnings cannot flip a case) but 34 of its
  inputs now carry one — the spec's own multi-book examples, and the
  `out_of_sequence_*` fixtures that are named for it; the benchmark corpus
  carries none and still parses with zero errors
- **Every `Code` audited, structure and tables moved (ticket 21, M4).** The
  rule is in `usfm_diagnostics`'s module doc as a table over all 55 codes: a
  code stays with the parser if deleting the check would change the tree, or
  if what the author wrote is no longer visible in it (a dropped token, a
  normalised number, a repair standing in for the input); everything else
  moves. Nine more codes are `usfm_semantic`'s — `missing-id`, `id-not-first`,
  `empty-book`, `verse-text-before-chapter`, `verse-outside-chapter`,
  `verse-in-heading`, `verse-in-character-style`, `unexpected-table-column`
  and `empty-word` — for 18 in all. `verse-in-note` stays (the verse is
  dropped, so it is not in the tree), `number-has-leading-zero` stays (`01` is
  the number 1), and `character-style-nested-without-plus` stays (it is the
  nesting decision). `ParserImpl::check_document_structure` is gone with
  `book`, `chapter_seen`, `para_is_s5` and `in_char_style`. `Document` gained
  a `span` (the source it was parsed from, `SPAN` for a hand-built tree) so
  `empty-book` can still point at the end of the file. Two rules changed where
  the tree is the better witness: a verse in a table cell is no longer "in a
  heading" because a heading came before the table, and `unexpected-table-column`
  names the column instead of the marker. Every diagnostic over the 385 inputs
  in the repo is otherwise unchanged; 27 of them have a wider span end, the
  node's rather than the marker's
- **The `usfm` facade and the ADR's layout (ticket 17, M3 closed).** One crate
  to depend on: `usfm` re-exports `span`, `style`, `ast`, `diagnostics` and
  `parser` unconditionally and `usx`, `html`, `json`, `pipeline` behind the
  features of those names (all four on by default), lifts `Document`,
  `ParseResult`, `Diagnostic`, `Code`, `Severity`, `Span`, `StyleSheet` and
  `DEFAULT_STYLESHEET` to its root, and adds `usfm::parse(&str)` and
  `usfm::parse_with(&str, &Arc<StyleSheet>)`. `apps/usfm_cli`,
  `tasks/conformance`, `tasks/benchmark` and `tasks/fuzz` depend on it instead
  of on the pieces (the features `benchmarking` and `testing` forward to
  `usfm_parser`'s); the crates under `crates/` keep depending on each other
  directly. Everything moved with it: `crates/usfm_*`, `tasks/conformance/`
  (the `usfm_tests` package, with its `fixtures/`, `tcdocs-patches/` and
  `tcdocs-baseline.txt`). `apps/usfm_cli`'s binary carries `doc = false`: it is
  also called `usfm`, and rustdoc would write it over the facade's page
- `usfm_json` (ticket 16): the AST as JSON, `to_json_value` /
  `to_json_string` / `to_json_string_pretty`. One object per node with
  `"type"` (the AST variant in snake_case, the whole vocabulary in
  `usfm_json::TYPES`), `"span": [start, end]` on every node (`[0, 0]` when
  synthesized), `"style"` as the marker name, `"children"`, `"attributes"` as
  `{"name", "value"}` pairs in source order (the default attribute keeps its
  empty name) and each node's own fields under their AST names; an absent
  optional field is left out rather than written as `null`. A recursion, not a
  `Visit`: every node maps to one `Value` and no state is carried between
  them. `usfm parse --format json` writes it on one line.
- The binary left the parser (ticket 15). `usfm_pipeline` holds the text
  replacements, the punctuation sectioning, the diglot HTML and the prompt
  weave, the SILE output and the format dispatch, each a function over
  `Document`s with a unit test; `apps/usfm_cli` is the `usfm` binary —
  `usfm parse <files> --format usx|html|json|usfm|sile|prompt`, `--stylesheet`,
  `--output`, `--replace`, `--diglot…`, `--watch`, `--strict`,
  `--deny-warnings`, `--diagnostics text|json` — on `clap`, tested by running
  it (`apps/usfm_cli/tests/cli.rs`). `usfm_parser` is a library with no
  binary, and depends only on `usfm_ast`, `usfm_style` and `usfm_diagnostics`
  (M3's exit criterion): its `::usx`, `::xml_document`, `::serialize_html`,
  `::serialize` and `::context` re-exports are gone, so name the output crate
  directly
- HTML output lives in `usfm_html` (ticket 14): `serialize_html.rs` and
  `context.rs` moved out of `usfm_parser` (`serialize.rs` moved too, and
  ticket 15 deleted it: the generic `Serialize` trait had no implementor).
  `Context` (note numbering, generated ids, the `metadata` map) is
  HTML state and went with them; its dead `from_book` and `marker` are
  gone. The writer escapes `&`, `<`, `>` in text and `"` as well in an
  attribute value, and replaces the characters HTML cannot carry with U+FFFD —
  the same set and the same byte-table scan as the USX writer, copied rather
  than shared because no output crate depends on another. `tasks/fuzz`'s
  `parse_html` target checks the markup is balanced and properly escaped
- USX output is well-formed XML for any input (found by the fuzz targets,
  ticket 06): the `XmlNode` writer replaces characters XML 1.0 cannot carry
  (the C0 controls, U+FFFE/U+FFFF) with U+FFFD in text and attribute values,
  and the serializers drop an attribute they cannot write — a name that is not
  an XML name (`malformed-attribute-name`) or a repeat of one already written
  (`duplicate-attribute`), both new `Code`s
- The CLI writes USX through the `usfm_usx` crate (`usfm_usx::to_usx_string`;
  `serialize_usx.rs` is gone), so it keeps word-level attributes. The walk is a `usfm_ast::visit::Visit`
  implementation over private state (book code, chapter, open verse,
  `include_vid`, the document's stylesheet) — no shared `Context`, and no
  dependency on `usfm_parser` (ticket 13). The `XmlNode` writer escapes text and
  adds no whitespace inside mixed content; `crates/usfm_parser/tests/usx_text.rs`
- Phase 3 traversal API (above); the old `usfm_parser::visit_mut::Context`
  is gone, `TextReplacement::apply_to(&mut document)` replaces
  `visit_mut_document`
- Verse and chapter ends are emitted by the parser as it goes (`OpenVerse` in
  `parser.rs`, plan D4) instead of by a path-arithmetic pass after parsing;
  the placement rules are one test each in `crates/usfm_parser/tests/verse_ends.rs`
- `Inline::OptBreak` for `//` (USX `<optbreak/>`, HTML `<wbr>`); the default
  attribute value is verbatim, quotes included, as Paratext reads it; a verse
  that starts in a non-verse-text paragraph (`\lit`) ends inside it when the
  previous verse started there too
- `StyleRule::occurs_under` carries the stylesheet's `OccursUnder` list. A
  note-only marker outside its note (`\xq` in a paragraph) is
  `marker-not-allowed-here` (Error); any other unlisted placement (`\f` under
  `\cl`, which Paratext accepts) is `marker-not-listed-here` (Info). Both are
  reported in `usfm_semantic` since ticket 20, over the style's whole node
- `TableCell::column` keeps the source column (`\th3` stays 3); a gap or
  out-of-order cell is `unexpected-table-column`, reported in `usfm_semantic`
  since ticket 21 (from the cells' columns in row order, so the message names
  the column rather than the marker). A verse that starts inside a
  non-verse-text paragraph (`\lit`) ends the previous verse before it
- Attribute validation: `empty-attribute-list`, `no-default-attribute`,
  `default-attribute-with-others`, `attribute-value-not-quoted`,
  `missing-attribute-value`; and `number-has-leading-zero` for `\c 091`/`\v 01`,
  which stays with the parser because `01` parses to the number 1 and the tree
  no longer has a zero to report.
  Seven `validated=fail` inputs now report an error instead of passing silently.
  `empty-attribute-list` (Error) is the character-style case only; on a
  milestone (`\ts-s |\*`, how unfoldingWord's aligned texts write a
  translation section) the same shape is `empty-milestone-attribute-list`
  (Warning, nothing dropped, same USX) — ticket 18. The six that keep the
  list as written (`empty-attribute-list`,
  `empty-milestone-attribute-list`, `no-default-attribute`,
  `default-attribute-with-others`, `malformed-attribute-name`,
  `duplicate-attribute`) are reported in `usfm_semantic` since ticket 20;
  the ones that decide what the list *is* stay with the parser
- `Milestone::attributes` is `Option<Attributes>` and `Char`'s already was:
  `None` is a marker with no `|`, `Some` with no pairs is `\ts-s |\*`. That is
  what `empty-milestone-attribute-list` reads, so `\zaln-s |\*` reports it too
  (ticket 20). `Attributes` keeps the `|`'s span and each `Attribute` its
  name's, so an attribute diagnostic still points at the attribute once the
  check reads only the tree
- `Block::Periph` (`\periph Title|id="x"` up to the next `\periph` or `\id`):
  a block container like `Sidebar`, with `title` and `attributes`; the
  attribute list on that line ends at the line break
- Character nesting without `\+` follows Paratext: a style nests iff its
  stylesheet entry allows `NEST`, the style it would nest *into* is not `\xo`,
  *and* its own closing marker lies ahead in the
  container (`character-style-nested-without-plus`, Info); otherwise it is a
  sibling (`character-style-implicitly-closed`). `\va`/`\vp` after `\v` become
  `alt_number`/`pub_number`; a `\vp` with formatting inside stays a `Char`
- `Block::Sidebar` (`\esb`…`\esbe`, the one block that contains blocks) with a
  `category`; `Note::category` from a leading `\cat`; milestones get their
  default attribute (`\qt-s |Speaker\*` is `who`). Verse ends stop at a chapter
  boundary and skip sidebars (plan D7)
- Whitespace rules pinned (rules 1–6 on `Text` in `usfm_ast`, tested in
  `crates/usfm_parser/tests/whitespace.rs`): only ASCII whitespace is normalised,
  `~` is a no-break space, trailing whitespace is dropped only at
  paragraph-level boundaries. The tcdocs harness ignores whitespace at note
  and cell ends on both sides because the reference files disagree there.
- `\usfm 3.1` sets `<usx version>`; the marker stays in the AST as a paragraph
  (`Document::usfm_version()`) and the USX serializers skip it
- Phase 1 complete: D1, D2 (spans), D3 (owned stylesheet), D5 (`into_owned`),
  D6 (attributes as a field, `Chunk` removed)
- Derived (`\k-s`) and unknown (`\zaln-s`, `\ts`) milestones are registered and kept
  rather than dropped; `Block::Milestone` for milestones between blocks
- `crates/usfm_parser/tests/spans.rs` checks span invariants mechanically; snapshots render spans
- Implemented word-level attributes (`\w word|lemma="value"\w*`)
- Added `Attributes` AST node type as last child of `Char`
- Attributes correctly serialized to USX as XML attributes
- Default attribute names based on marker style (w→lemma, rb→gloss, xt/jmp→link-href)
- Added `vid` attribute to `<para>` elements continuing a verse from previous paragraph
- Fixed verse end milestone placement with footnotes/notes
- Fixed path tracking in ChapterVerseMilestoneInsertion visitor for nested note children
- Implemented table support (`<table>`, `<row>`, `<cell>` elements with proper styles)
- Tables now correctly handle `vid` attribute and verse end placement

**Phase 1 is complete (2026-09-12).** The AST shape is frozen apart from additive
Phase 2 conformance changes (D7: `Block::Sidebar`, `Block::Periph`, `Note::category`); Phase 2
(conformance) and Phase 3 (traversal) can proceed. What changed, and the two traps
it leaves for callers:

- D3: `Document` owns an `Arc<StyleSheet>` and nodes hold `StyleId`. **When
  consuming a `Document`, build your `Context` from `document.style_sheet()`** — not
  from `DEFAULT_STYLESHEET` or your own sheet, which may lack styles the parser
  derived.
- D2: every node has a `span`. **A `Text` span covers the source run it was read
  from; `&source[text.span]` is not expected to equal `text.content`** (whitespace
  is normalised, escapes resolved, trailing whitespace trimmed). Synthesized nodes
  carry `SPAN`. `Span`, `SPAN` and `LineIndex` (line/column lookup, built once per
  source) live in the `usfm_span` crate, re-exported as `usfm_ast::span` (ticket 11).
- D6: `Char::attributes` is a field; `Inline::Attributes` and `Chunk` are gone.
- D5: `Document::into_owned()` detaches a parse from its source.

Priority areas, next: the route is `.scratch/oxc-layout/spec.md` (decision in
`docs/adr/0001-oxc-style-crate-layout.md`): oxc's crate organisation, not its arena.
Milestones in order: M1 workspace builds clean, M2 benchmarks + Miri + fuzz, M3
crate split, M4 `usfm_semantic`, M5 `usfm_codegen`, M6 language server. M1–M5
closed (exit criteria recorded in the spec; M5 on 2026-09-20, tickets 25–27
and 34–36). Ticket 37 paid back the M5 parse regression, so **next is M6**,
the language server rebuilt in `apps/` (tickets 30–33, starting with 30).
Tickets are in
`.scratch/oxc-layout/issues/`, written one milestone ahead. Unattended
sessions follow `docs/agents/loop.md`; `scripts/gate.sh` is the gate before every push.

## Boundaries

Minimal restrictions - work freely as long as changes are revertable:
- Can create PRs
- Can modify grammar
- Can run tests (`cargo test`)
- Can commit changes
- `main` is protected (PR + green `test` check, no bypass). Work on a branch,
  open a PR, merge when CI is green; see `docs/agents/loop.md` step 5

## Project Structure

The layout is the one in `docs/adr/0001-oxc-style-crate-layout.md`, reached by
ticket 17 (M3, 2026-09-19). Dependencies point one way: span <- style, ast <-
diagnostics <- {parser, semantic} <- outputs <- pipeline <- facade <- apps. No
output crate depends on another output crate, and `usfm_semantic` does not
depend on `usfm_parser`: a check there is a function of a `Document` and its
stylesheet, whoever built them.

```
usfm-tools/
├── crates/
│   ├── usfm_span/         # Span, SPAN and LineIndex (leaf crate, no dependencies)
│   ├── usfm_style/        # The stylesheet: StyleSheet, StyleRule, StyleId
│   ├── usfm_ast/          # AST nodes, Visit / VisitMut / Fold, Cursor, PlainText
│   ├── usfm_diagnostics/  # Diagnostic, Code, Severity, ParseResult, rendering
│   ├── usfm_parser/       # Lexer + parser. A library, no binary
│   ├── usfm_semantic/     # Checks over a finished tree: analyze(&Document)
│   │                      #   -> Vec<Diagnostic>. Reports, never repairs.
│   │                      #   Also ReferenceIndex, the chapter/verse index
│   ├── usfm_usx/          # AST -> USX: the XML tree, its writer and reader
│   ├── usfm_html/         # AST -> HTML: ToHtml, SerializeHtml, Context
│   ├── usfm_json/         # AST -> JSON: the tree as it is, one object per node
│   ├── usfm_codegen/      # AST -> USFM: one canonical spelling per construct,
│   │                      #   `to_usfm_string`. Round-trips the tcdocs corpus
│   ├── usfm_pipeline/     # Document -> Document/text: replacements, sections,
│   │                      #   diglot, prompt, SILE, the format dispatch
│   └── usfm/              # Facade: re-exports the above behind features, and
│                          #   `usfm::parse` / `parse_with` / `parse_with_options`,
│                          #   whose diagnostics are the parser's plus
│                          #   usfm_semantic's. `apps/` and `tasks/` depend on
│                          #   this, not on the pieces
├── apps/
│   └── usfm_cli/          # The `usfm` binary: clap, watch mode, diagnostics
├── tasks/
│   ├── conformance/       # tcdocs + usfm-grammar runner (the `usfm_tests` crate),
│   │                      #   its fixtures, patches, baseline and the
│   │                      #   round-trip property (`roundtrip.rs`) all three
│   │                      #   round-trip checks share
│   ├── benchmark/         # criterion benches over a committed corpus
│   └── fuzz/              # cargo-fuzz targets (own workspace, nightly)
├── tcdocs/                # Git submodule: official USFM test suite
└── wip/                   # Outside the workspace, parked until M6, does not build
    ├── usfm_language_server/  # LSP server (parked)
    └── data_layer/            # Data persistence (parked)
```

Every crate the ADR's tree calls for now exists: `crates/usfm_semantic`
arrived with ticket 19 and `crates/usfm_codegen` with ticket 25. M6 rebuilds
the language server under `apps/`.

## Running Tests

```bash
# Run all tests with summary
cargo run --package usfm_tests

# Run specific category
cargo run --package usfm_tests basic
cargo run --package usfm_tests mandatory

# List categories
cargo run --package usfm_tests -- --categories

# Gate against the known-failure list (what CI runs)
cargo run --package usfm_tests -- --baseline tasks/conformance/tcdocs-baseline.txt

# Regenerate the list after fixing or regressing cases
cargo run --package usfm_tests -- --write-baseline tasks/conformance/tcdocs-baseline.txt

# The round trip over every case of every root, gated the same way (ticket 27)
cargo run --package usfm_tests -- --roundtrip tasks/conformance/roundtrip-known.txt
cargo run --package usfm_tests -- --write-roundtrip-known tasks/conformance/roundtrip-known.txt

# Run as cargo test (with output)
cargo test --package usfm_tests -- --nocapture
```

## USFM Documentation

Reference docs are a local, git-ignored copy in `.claude/docs/` (not
redistributed; fetch with `python3 .claude/import_docs.py`):
- `QUICK_REFERENCE.md` - Consolidated parsing reference
- `about/syntax.md` - Detailed syntax rules
- `paragraphs/index.md` - Paragraph markers
- `characters/index.md` - Character markers
- `attributes/index.md` - Word-level attributes
- `notes_basic/fnotes.md` - Footnotes
- `notes_basic/xrefs.md` - Cross references

To fetch or update docs: `python3 .claude/import_docs.py`

## Context

- Custom lexer/parser (not tree-sitter)
- Workspace with 6 crates
- Test suite from usfm-bible/tcdocs (git submodule)
- Tests validate USFM → USX (XML) conversion
- USFM spec: https://ubsicap.github.io/usfm/

## Agent skills

### Issue tracker

Issues live as local markdown files under `.scratch/<feature>/` in this repo. See `docs/agents/issue-tracker.md`.

### Triage labels

Default vocabulary: `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: one `CONTEXT.md` and `docs/adr/` at the repo root. See `docs/agents/domain.md`.
