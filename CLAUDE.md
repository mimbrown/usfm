# usfm-tools

Rust-based USFM parser with AST, language server, and multiple output formats.

## What "Progress" Means

This project has three parallel work streams, all in progress:

1. **Parser Development** (`crates/usfm_parser/`, `crates/usfm_ast/`)
   - Parse USFM into AST structure
   - Handle edge cases and malformed input gracefully
   - Expand grammar coverage

2. **Language Server** (`apps/usfm_language_server/`) — M6, in progress
   - Powers the VS Code extension in `vscode/` (binary `usfm-language-server`)
   - Live since ticket 30 (2026-09-20): full text sync, a document store and
     `publishDiagnostics` from `usfm::parse_with` — the parser's diagnostics
     and `usfm_semantic`'s, the same list `usfm parse` prints
   - And since ticket 31: `textDocument/formatting` (`usfm_codegen`'s text,
     one edit over the whole document, refused on an Error) and
     `textDocument/hover` (the stylesheet's `\Name`, `\Description` and
     `\OccursUnder` for the marker under the cursor, or the reference —
     `GEN 1:1` — in a verse)
   - And since ticket 32: `textDocument/documentSymbol` (the outline —
     book, chapters, verses, sidebars and `\periph` divisions),
     `textDocument/completion` (the markers that may stand where the cursor
     is, from the document's own sheet, `\` the trigger character) and
     `textDocument/codeAction` (a `quickfix` per parser repair that has an
     obvious edit)
   - The stream's M6 tickets are done, and M6 is closed (2026-09-26). The
     old server it replaced is deleted (ticket 33), and the lexicon feature
     that server had — a SQLite word list checked with regexes — was
     decided against on 2026-09-26 (ticket 33): it is not coming back for
     now. Whatever comes next, advertise a capability only once it works,
     and keep the diagnostics path as it is — the server holds no rules of
     its own, it publishes the toolchain's

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
(ticket 19); and every rule only the USX reader has (`usx-…`, ticket 45) is
one with a test in `crates/usfm_usx/tests/reader.rs`. `Code::origin()` —
`Origin::{Parser, Semantic, Usx}` — says which file covers a code, the three
files between them cover every `Code`, and each one's coverage test reads
`origin()` to know which third is its own (`is_semantic()` is
`origin() == Origin::Semantic`). Only
`usfm::parse` / `parse_with` / `parse_with_options` return both halves;
`usfm_parser::Parser::parse` returns the parser's alone. For USX the same
holds of `usfm::parse_usx` / `parse_usx_with` (ticket 48) and
`usfm_usx::read_usx`.

Conformance status (276 tests across two roots, 2026-09-27):
- tcdocs (260 tests): 216 passed, 0 failed, 0 panicked, 1 skipped,
  43 expected failures, 0 unexpected passes
- `usfm-grammar/bugfixes` (16 tests, vendored under
  `tasks/conformance/fixtures/usfm-grammar/`, MIT): 16 passed, 0 failed
- `tasks/conformance/fixtures/machine-py/` (sillsdev/machine.py, MIT) is *not* a harness
  root: those Paratext-shaped projects ship no reference USX, so their
  expectation is the parser's own tree and diagnostics, snapshotted by the
  `machine_py_*` tests in `crates/usfm_parser/tests/recovery.rs`. All seven books are
  fuzz seeds. Its `usx/` (ticket 47, same commit) is real USX no tool of ours
  wrote, for the reader: the World English Bible's 1–3 John from a DBL
  release (USX 3.0, every word a `<char style="w">`, pretty-printed inside
  paragraphs; public-domain text) and `Tes/MAT.usx`, a USX 2.6 book malformed
  on purpose, with `Tes/MRK.usx`. Their trees and diagnostics are the
  `machine_py_*` snapshots in `crates/usfm_usx/tests/reader.rs`: the WEB books
  report nothing and round-trip USX -> USFM -> USX (`usx_reader.rs`); the Tes
  book reports its implicit `\p` and fifteen `usx-verse-end-missing`.
- `tasks/benchmark/corpus/aligned/` (unfoldingWord's ULT and UGNT of Acts, from
  usfm-js's test resources, **CC BY-SA 4.0** — not MIT; its own `LICENSE` and
  README, and a `NOTICE.md` entry) is not a harness root either: it is the
  benchmark's `aligned` class (ticket 10), a round-trip input and two fuzz
  seeds. Its files are upstream's with every milestone closed (`\*`), since
  usfm-js's old format leaves them open, which is not USFM 3.
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
- Fifteen reference files (ten tcdocs, five usfm-grammar) are read through a
  patch in `tasks/conformance/tcdocs-patches/`
  (rules in its README; the directory covers both roots): a unified diff with
  the rationale above it, for a reference quirk or an accepted deviation, never
  for a parser gap. The harness fails a patch that stops applying or that the
  parser no longer needs. `cargo run -p usfm_tests -- --show <name>` prints one
  test's diagnostics, output and patched expected USX.
- `crates/usfm_style/usfm-extra.sty` is appended to Paratext's `usfm.sty` by
  `crates/usfm_style/build.rs` into `usfm_style::DEFAULT_STYLESHEET` (there
  since ticket 45, which moved it out of `usfm_parser` so the USX reader can
  resolve styles without depending on the parser; `usfm_parser` and the
  facade re-export it under the old name): the markers that sheet predates (`\ipc`, `\ta`,
  `\wl`) and the one entry it gets wrong (`\xta` occurs under `\ex` too),
  each citing `tcdocs/grammar/usx.rnc`, plus `\s5`, unfoldingWord's chunk
  break, which no schema lists. `usfm.sty` itself is byte for byte tcdocs'
  `grammar/usfm_sb.sty` (CC BY 4.0, `NOTICE.md`): add to the extra sheet,
  never to it.
- AST snapshot corpus: `cargo test -p usfm_parser --test snapshot` (review with `cargo insta review`)
- Run `git submodule update --init tcdocs` first. Without it the `usfm_tests` build
  fails on purpose, so a zero-test run can never report a pass rate.
- CI (`.github/workflows/ci.yml`) runs the unit and integration suites
  (`recovery`, `snapshot`, `spans`, `whitespace`, `attributes`, `verse_ends`, `usx_text`,
  `usfm_html`'s `footnotes`, `usfm_json`'s `json` and `coverage`,
  `usfm_semantic`'s `checks`, `usfm_codegen`'s lib and `roundtrip`,
  `usfm_usx`'s `reader` and `writer`,
  `usfm_language_server`'s unit tests and its `lsp` conversation with the
  built binary, parser lib),
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
  needs a reason naming the bug. After it come M7's two USX properties
  (ticket 46), over the 229 cases the harness compares (an input and a
  reference, and the parse matched it), each written as USX and compared
  with the reference exactly as the harness compares — patches applied, its
  whitespace rules — and each gated the same way against an empty list:
  `--usx-read tasks/conformance/usx-read-known.txt` reads the reference with
  `usfm_usx::read_usx` and writes it back, and
  `--usx-roundtrip tasks/conformance/usx-roundtrip-known.txt` reads it,
  writes USFM with `usfm_codegen`, parses that and writes USX. Both are on
  the USX side, so `usfm_codegen`'s canonical spellings are invisible to
  them; both are defined in `tasks/conformance/src/usx_properties.rs`, and
  `--show <name>` prints the reader's diagnostics over the case's reference
  and whether each holds. The 42 `fail` cases the harness never compares are
  not covered: their references carry Paratext's `status="invalid"` /
  `"unknown"`, `<unmatched>` and a `sid` with no book code, which the tree does not
  model. Then (ticket 45) `cargo test -p usfm_tests --test usx_reader`, the
  USX reader over every reference of both roots: each reads without a panic
  and a `pass` one with no Error; for every case the harness compares, the
  tree is `usfm::parse` of the case's USFM up to `usfm_usx::testing::normalise`
  — what USX cannot say, behind `usfm_usx`'s `testing` feature since ticket
  49 so the fuzz target compares through the same list: the default
  attribute's name, one `<usx version>` as the `\usfm` paragraph after
  `\id`, a note's trailing whitespace, an attribute the writer cannot carry
  and the empty `|`, a character XML forbids as U+FFFD — and whitespace on
  the usfm-grammar root; every read node's span is in its source
  (`usfm_parser::span_check::read_violations`, shared with `read_usx`) (writing
  back to the reference moved to `--usx-read`, so it is asserted once); every
  reference with its `<verse eid>`s and `<chapter eid>`s stripped reads to the
  tree the reference reads to (ticket 47: USX 2 has no `eid`, and the reader
  builds the ends by the parser's rules, mirrored in `read.rs` rather than
  shared because the parser places them while it builds; one `fail`
  reference, `usfmjsTests/invalid`, is on a known list with its reason); and
  the machine.py WEB books round-trip. Since the reader builds the ends a USX 2
  file leaves out, `--usx-read` drops them before writing when the reference
  has none, as the harness parses such a case without them
  (`advanced/complex`). A file with some `eid`s and not others is the
  malformed case: each end it lacks is built the same way and reported as
  `usx-verse-end-missing` (Warning). Then
  `scripts/third_party_notices.py --check --no-npm`: every crate the shipped
  binaries link carries a licence file, since the `.vsix`'s
  `ThirdPartyNotices.txt` (`npm run notices` in `vscode/`) quotes them. Last in the gate is `scripts/miri.sh`
  (ticket 05): `cargo +nightly miri test` over the `usfm_span`, `usfm_ast` and
  `usfm_diagnostics` libs, `usfm_usx`'s lib and (8 of 25, since ticket 49)
  `reader` tests, `usfm_html`'s `escape` tests, the parser lib
  and the `whitespace`, `attributes`, `usx_text`, `verse_ends`, `spans` and
  (10 of 103) `recovery` suites, about 5 min (2026-09-27; about 4 min 30 s
  without the reader, the rest of the growth being drift on this VM since the
  last measurement — the whole of the script's own 5-minute budget, see its
  header). It needs a nightly toolchain
  with `miri` and `rust-src`, which CI installs in its own step and
  `scripts/session-start.sh` installs on a fresh VM; `rust-toolchain.toml`
  stays pinned at 1.98.0 for everything else.
- Fuzzing is on demand, not in the gate or CI (ticket 06): `cargo +nightly fuzz
  run --fuzz-dir tasks/fuzz parse_lossy -- -max_total_time=600 -max_len=65536`,
  and the same for `parse_utf8`, `parse_html`, `roundtrip`, `read_usx` and
  `usx_roundtrip`. The first two
  assert no panic,
  the span invariants (`usfm_parser::span_check`, shared with `crates/usfm_parser/tests/spans.rs`
  behind the `testing` feature) and that the USX output is well-formed XML;
  `parse_html` (ticket 14) asserts no panic and that the HTML output is balanced
  and properly escaped, checked by a scanner in `tasks/fuzz/src/lib.rs` rather
  than by an HTML parser; `roundtrip` (ticket 27) parses, writes USFM and parses
  again, asserting the same three things the gate's `--roundtrip` step does —
  the tree is equal ignoring spans, the second parse gains no diagnostic code,
  and the output is a fixed point. `read_usx` (ticket 49) reads arbitrary
  bytes as USX through `usfm::parse_usx`: no panic, the span invariants as
  they read for XML, well-formed USX out; `usx_roundtrip` parses arbitrary
  USFM, writes USX, reads it back to the parse up to
  `usfm_usx::testing::normalise`, then holds `roundtrip`'s three assertions
  over the read tree. Seeds for `read_usx` are every reference USX of both
  roots and the vendored machine.py USX. `tasks/fuzz` is
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
- **Standard reference punctuation is kept unless claimed (2026-10-06).**
  A real project sets `ChapterVerseSeparator` to `.`, sets no other
  reference punctuation, and writes `:` in every reference, so not one of
  its `\xt` was read from text. `usfm_paratext`'s
  `Settings::citation_format` now gives each setting the file declares its
  standard spelling as well (`:` chapter–verse, `-` range, `—`/`-` chapter
  range, `,` sequence, `;` chapter and book) **unless the project declares
  that spelling for a different setting** — so `3,16` with `.` as the
  sequence mark gains `:` and never `,`, and a project that keeps `-` for
  verse ranges and `—` for chapter ranges keeps them apart. A setting the
  file leaves out is the default, as before. Over the real projects: that
  one went from 0 citations to 74, and no other project's count moved.
  Whether Paratext itself does this is not known (Michael's hunch); the
  rule stands on having one reading. It is silent, and Michael wants it a
  Warning once the citation reader has somewhere to report (spec, "Work
  with no plan yet"). Most of that project's references stay unread for
  another reason — its `BookNames.xml` names three books — which is the
  project's to fix
- **The stylesheet's `\Attributes` (2026-10-06).** A real project writes
  its own milestone with an unnamed value and got `no-default-attribute`,
  and nothing it could put in its `custom.sty` would have helped: the sheet
  parser dropped `\Attributes` and the default attribute was a table keyed
  by marker name in `usfm_ast`. `StyleRule::attributes` now keeps the line
  (`?name` optional, `name` required; `usfm.sty` writes it `#!\Attributes`,
  which the reader already unwrapped), and
  `StyleRule::default_attribute()` / `StyleSheet::default_attribute(marker)`
  answer by Paratext's rule, checked against sillsdev/machine.py: the first
  attribute listed, unless more than one is required (`\fig` has none). A
  rule that declares nothing falls back to the old table, now
  `usfm_style::builtin_default_attribute` — `ref`, `periph`, `tl`, `wl`,
  `vid` have no `\Attributes` in the sheet, and a replaced sheet may
  predate the line. `usfm_ast::default_attribute_name` is gone; the USX
  writer, `usfm_usx::testing::normalise` and the `no-default-attribute`
  check ask the document's sheet. One default is new: `\ts-s` declares
  `?sid ?eid`, so `\ts-s |x\*` is `sid="x"` where it was an Error. A rule
  the parser derives (`\k-s` from `\k`) inherits no attributes. A marker
  a project uses and does not declare still has no default — that is the
  project's to declare, and nothing is guessed
- **`usfm fix`, and the fixes in one place (2026-10-06).** Michael's shape
  for a linter with auto-fix: a dry run by default, every fixable
  diagnostic fixed unless specific codes are named, and the editor a
  pass-through. `crates/usfm_fix` (`usfm::fix`, no feature: it needs only
  the AST, diagnostics and spans, and does not parse) now holds the five
  fixes ticket 32 wrote in the language server — `fix(document, source,
  diagnostic) -> Option<Fix>`, `FIXABLE` / `is_fixable`, `fixes(…, only)`
  for a whole parse, and `apply(source, &fixes)`, which writes them in one
  pass and leaves out any whose edits touch one already taken. A `Fix`
  carries its diagnostic's `code` and `span`. The server's `actions.rs` is
  a re-export; its tests moved to `crates/usfm_fix/tests/fixes.rs`.
  `usfm fix <files> [--code CODE]… [--write]` (`apps/usfm_cli/src/fix.rs`)
  is per file like `format`: it lists `file:line:col: fix[code]: title` on
  stdout and a count on stderr, writes nothing without `--write`, parses
  and asks again until no fix is left (a fix can uncover another; 16 passes
  at most, and a file that does not settle is left alone), and refuses a
  `--code` that is unknown or has no fix. Exit 0 whether or not fixes were
  available. **A new fix is a `Code` in `FIXABLE`, an arm in `fix`, and a
  test in `fixes.rs` that applies it and re-parses** — the server and the
  CLI get it for nothing. Next: the interlinear repair as a fixer of its
  own (it is not a USFM diagnostic, so it will not go through `fix`)
- **`\xt` citations: read what is unambiguous, guess nothing (2026-10-06).**
  Michael's rule, from the run over real Paratext projects: wrong parsing
  is worse than no parsing. Four changes, all in or under
  `usfm_semantic::citation`. (1) `xt_citations` believes a `link-href`:
  `XtCitations::link` is `Some(LinkHref { value, citations })` when the
  `\xt`'s default or `link-href` attribute is references and nothing else
  (`parse_link_href`: codes in any case, direction marks skipped; a URL or
  `prj:` link is `None`), and `XtCitations::citations()` answers with the
  link's when there is one and the text's otherwise. `pieces` stays the
  text's own reading. (2) A `C:V` with no book name is no longer given the
  default book when an unread word stands directly before it — that word
  may be a book the table lacks, and was in about 470 `\xt` of the
  projects, each read as the book it stood in. `see 3:16` goes unread with
  it; `(3:16)` and `cf. 3:16` do not. (3) Whitespace may follow the
  chapter–verse separator (`23: 5-6` read as chapter 23 alone).
  (4) `usfm_paratext`'s `BookNames::table()` reads a `~` in a name as
  U+00A0, which is what the parser has made of the text's `~`: one
  project names its Gospels that way, and 945 references to them were
  being read as the current book. A bare number in an `\xt` stays unread:
  in this data it is a verse 18 times and a chapter 23, told apart only by
  the word beside it. Decided, not open: a `C:V` alone in an `\xt`
  with no `link-href` is the current book (Michael, 2026-10-06). Where a
  `link-href` was there to check, it named a different book 77 times, the
  name standing outside the `\xt` — that is the source text's to fix
- **`content-outside-paragraph` is a Warning (2026-10-06, Michael's
  call).** It was an Error, and the same run over real Paratext projects
  reported it 1 300 times in 54 books, every sampled one a `\c N` followed
  directly by `\v 1`: nothing is lost and the implicit `\p` is the tree the
  author meant. Only `Code::severity` changed — no tree, message or span, and
  no conformance case moved. What follows from it: `--strict`,
  `usfm format --write` and the server's formatting no longer refuse a file
  for this alone (`--deny-warnings` still does), and machine.py's
  `Tes/MAT.usx` now reports no Error at all. A lint configuration file, when
  there is one, must be able to turn the code off (spec, "Work with no plan
  yet")
- **Direction marks in a verse number (2026-10-06).** Found by running the
  toolchain over seven real Paratext projects (231 books; every one parsed,
  ten failed the round trip, all on this). `NumberRange` keeps a `guard`
  (the mark before the `-`, `1<RLM>-3`, Paratext's bridge spelling) and a
  `trailing_guard` (after the range: `4<RLM>`, `1-3<RLM>,5`), each an
  `Option<char>` holding U+200F or U+200E (`usfm_ast::is_direction_mark`),
  written back exactly where it was read. They replace the two `guard_rtl`
  bools, which remembered a trailing mark only as "write one before every
  comma": `\v 4<RLM>` came back as `\v 4`, silently. U+200E was not read at
  all — `\v 4<LRM>` was `malformed-verse-number` and the verse dropped. One
  mark per place; anywhere else (before the number, doubled, any other
  invisible character) is still malformed. The mark is part of the number,
  so it is in USX's `number`, `sid` and `eid`, as a bridge's always was
- **What render needs (tickets 51–57, 2026-09-27).** Michael's older
  publishing project, Shahkar-Urdu-Apps/render, was mapped against this
  toolchain (`reports/render-gap-map.md` in the project files; the spec's
  "After M7: what render needs"). Ticket 51 landed with the tickets:
  `usfm parse` and `usfm format` take `--custom-stylesheet FILE`, a
  project's `custom.sty` read *over* the `--stylesheet` sheet or the
  built-in one (`StyleSheet::extend_from_str`, the language server's
  semantics), where `--stylesheet` alone replaces it. Ticket 53 added
  `crates/usfm_paratext` (`usfm::paratext`, feature `paratext`, default
  on): `Project::open(dir)` reads `Settings.xml` and `BookNames.xml`,
  finds each book's file by the project's naming rule (`41MATSSV.SFM`,
  Paratext's file numbers in `canon`), reads it (UTF-8 only) and builds
  the project's sheet — the default with its `custom.sty` over it. It
  parses nothing: hand `read_book` and `style_sheet` to `parse_with`.
  Ticket 54 added `usfm_semantic::citation`: `parse_citations` reads
  references written as text (`Mt 5:3-10; Lk 6:20`, `متی ۵:۳، ۷`) in a
  project's own punctuation (`CitationFormat`, from
  `Settings::citation_format()`) and book names (`BookNameTable`, from
  `BookNames::table()`), any Unicode decimal digits, and `xt_citations`
  runs it over every `\xt` of a document. Ticket 55 added
  `usfm_semantic::GlossaryIndex` (`usfm::GlossaryIndex`): every `\k` of
  the documents it is built from (the glossary is usually its own book),
  and for each `\w` of a document the entry its lemma or text names,
  compared whitespace-collapsed and lower-cased. Michael answered 52 and 56
  on 2026-09-28: 52 needs no code (render's `\z…` and `\app-…` markers are
  in its `custom.sty`, which 51 and 53 load). 56 is `usfm_paratext`'s
  reader for Paratext 9's interlinear glosses: `InterlinearBook`
  (`Interlinear_{lang}_{book}.xml`), `Lexicon` (`Lexicon.xml`),
  `Project::interlinear` / `lexicon`, and `anchor::anchor(text, clusters)`,
  which places a verse's clusters on the words of `VerseRef::text()` **by
  form** — a cluster's `Range` counts Paratext's own verse string and goes
  stale when the text changes, so it only orders and breaks ties — after
  SIL's interlinearizer extension, whose four invented MIT test projects
  are the fixtures (`tests/fixtures/pt9/`, `NOTICE.md`). 57, a publishing
  layer, is specified as the spec's "Publishing foundation" (2026-09-28):
  render moves to Rust end to end and keeps its own model (`RenderBlock`,
  a cartouche over several `\m#` paragraphs) over our `Block`s — no AST
  variants — and usfm offers writers driven block by block (tickets
  59–61, done: `usfm_usx::UsxWriter` with `block`, `open`/`close`/`empty`
  and `UsxHooks`, one associated function per node kind with the writer's
  `write_*` as its default; `usfm_json::JsonWriter`'s per-node values; and
  HTML's `ToHtml` per block with one shared `Context`, pinned by a test),
  scoped edits (62) and chapter selection (63); a parser
  in WASM driven from JS, as oxc does, is ticket 64. Ticket 58 is done:
  `Document` and every AST node are `Clone`, as a convenience only: the
  print and digital pipelines *borrow* one edited document and each builds
  its own model from it, so the design never needs a copy
- **Markers delimited by a line break or the next `\` (hardening plan,
  Phase 2).** Nothing changed in the parser: fourteen such spellings already
  parsed with no diagnostic to the tree of their one-space spelling, and
  `whitespace.rs` now pins it, so the plan's box is ticked. Of its unchecked
  boxes, a malformed real-world corpus and a benchmark gate in CI are left
- **USFM 2's `\fig`, and a second `|` (hardening plan, Phase 2).**
  `\fig DESC|FILE|SIZE|LOC|COPY|CAP|REF\fig*` reads as Paratext converts it:
  `CAP` is the caption and the rest are `alt`, `src`, `size`, `loc`, `copy`
  and `ref`, an empty field left out, reported as `usfm2-figure` (Warning,
  parser: the positional form is not in the tree). It had kept `DESC` as the
  caption and only the last field, silently, because every `|` in a
  character style started a list that *replaced* the one before. Only the
  exact shape is taken (plain text, six `|`, `\fig*`: `take_usfm2_figure`);
  anywhere else a second `|` is now `unexpected-pipe` and kept as text, so
  `\w a|lemma="x"|strong="y"\w*` keeps `lemma` and reports it.
  `paratextTests/EmptyFigure` is compared now (the empty USFM 2 figure is
  what its reference has) and passes through a patch for its other figure,
  `\fig  |…`, whose second space rule 6 gives to the marker
- **Fuzz, Miri and a benchmark for the USX reader; M7 closed (ticket 49).**
  Two fuzz targets: `read_usx` (arbitrary bytes through `usfm::parse_usx`;
  no panic, `span_check::check_read`, well-formed USX out) and
  `usx_roundtrip` (parse, write USX, read back to the parse up to
  `usfm_usx::testing::normalise`, then `roundtrip`'s three assertions over
  the read tree, comparing codes with the *read's* diagnostics). Seeds are
  every reference USX of both roots and machine.py's USX, NIV excluded; both
  ran ten minutes clean. The discovery rounds before that found seven bugs,
  each a test before its fix (the table is in `tasks/fuzz/README.md`):
  `<para style="esbe">` now reports `unmatched-sidebar-end`; a verse that
  starts inside a character style of a non-verse-text paragraph ends in that
  paragraph; the pending verse end is a **stack**, so a second waiting end
  no longer overwrites the first (both mirrored in parser and reader); the
  reader takes `\thc0`/`\tc3-2` cell styles as the parser does; the USX writer
  escapes attribute values itself (`xml-rs` wrote a tab raw, which a reader
  reads as a space); the nesting look-ahead counts a `\+name*` as the closer
  of a `\+name`, not of an unplussed `\name`; and **`\usfm` is read like
  `\id`** — its version and nothing else — where it had run on like any
  paragraph and swallowed a milestone or a verse that the USX writer then
  dropped. `normalise` moved out of the conformance test into
  `usfm_usx::testing` (feature `testing`, forwarded by the facade's) so the
  test and the fuzz target compare through one list. `scripts/miri.sh` runs
  the `reader` suite; `tasks/benchmark` has a `read_usx` group (84.1 MiB/s of
  USX over the whole corpus, about 1.2× the parser's time for the same
  books); the M7 boundary rerun against `8f8f568` is within noise
  (`docs/benchmarks.md`, "M7 close")
- **`usfm::parse_usx` and `usfm parse --from usx` (ticket 48, M7).** The
  facade's `parse_usx` / `parse_usx_with` (feature `usx`) are `read_usx` /
  `read_usx_with` plus `usfm_semantic::analyze`, merged the way `parse_with`
  merges (one private `with_semantic` serves both). The CLI's
  `parse --from usfm|usx` (default `usfm`; a `.usx` extension never
  switches it) applies to the `--diglot` files too, so
  `usfm parse book.usx --from usx --format usfm` converts and
  `--format usx` normalises; every other flag is unchanged. Several USX
  files cannot be concatenated as text — each is its own XML document — so
  `driver.rs` reads each on its own, prints its diagnostics under its own
  path with positions in its own XML (a USFM run files them under `input`,
  the joined source), applies `--strict` / `--deny-warnings` after every
  file has reported, and `combine`s the documents block by block, moving a
  style one file derived onto the combined sheet's entry for that marker.
  The combined tree's `span` is `SPAN`. `usfm format` stays USFM-only: it
  has no `--from`, which clap refuses (exit 2). The language server stays
  USFM-only
- **Real aligned USFM in the benchmark corpus (ticket 10).** Michael's answer
  was "vendor them": `tasks/benchmark/corpus/aligned/` holds unfoldingWord's
  Acts from usfm-js's test resources (commit `0ecae6f`, fetched 2026-09-26),
  the English **ULT** with every word in `\zaln-s`/`\zaln-e` (`large.usfm`
  upstream, 3.9 MB) and the Greek **UGNT** it is aligned to, every word a
  `\w` with `lemma`/`strong`/`x-morph` and 156 `\k-s` key terms
  (`45-ACT.ugnt.oldformat.usfm`; the `45-ACT.ugnt.usfm` the ticket named is a
  130-byte placeholder upstream). **CC BY-SA 4.0**, with the licence's legal
  code as `LICENSE`, a README and a `NOTICE.md` entry. Both upstream files are
  usfm-js's *old format* — a milestone's attribute list runs to the line's end
  with no `\*`, which tcdocs judges `fail` and which parsed as 19 140
  `unexpected-pipe` Errors with every attribute list kept as text — so what is
  committed is upstream passed through `tools/usfmjs_oldformat.py`, which
  appends the `\*` and checks it changed nothing else. The parser was right
  about the old shape, and since ticket 43 it recovers it: a `-s` milestone
  whose attribute list reaches the line's end with no `\*` is closed there,
  keeps its attributes and reports one `milestone-not-closed`, so upstream's
  files parse to exactly the committed files' trees with 19 140 and 156
  Errors (`recovery.rs`'s
  `the_old_format_aligned_books_read_as_their_closed_spelling`). The
  closed files report no Error: 28 `content-outside-paragraph`
  (the ULT never writes a `\p` after `\c`; a Warning since 2026-10-06) and Infos. The class is **`aligned`**,
  replacing the synthetic `alignment-heavy` Luke, which is no longer committed
  (`synthesize.py --class alignment-heavy` still writes it byte for byte;
  `attributes-heavy` is unchanged); `whole-corpus` is now 91 files, 14.36 MiB,
  so no earlier `whole-corpus` number compares with a later one. Measured in
  one sitting against the synthetic Luke, `parse` reads the real text at
  90.2 MiB/s against 88.4 — the synthetic class was a fair stand-in
  (`docs/benchmarks.md`, "usfm-js aligned corpus"). Both books round-trip
  (`the_benchmark_corpus_round_trips` reads `aligned` beside `web/`) and are
  fuzz seeds as `usfm-js__*`
- **Symbols, completion and code actions (ticket 32, M6).** Three more
  capabilities, three more modules of pure functions with the wiring in
  `main.rs`. **`textDocument/documentSymbol`** (`symbols.rs`) is the outline:
  the `\id` at the top, chapters under it, verses under those, and a sidebar
  or a `\periph` where it stands — kinds Module / Namespace / Key / Object.
  Entries are built flat, each with the source range it covers, and nested by
  **containment**, which is what the protocol requires of a child's range and
  what puts a sidebar inside the verse it interrupts without a special case.
  `selection_range` is the marker (`\c 1`), `range` the content: to the next
  chapter or verse, cut short by the end of the block container the node is
  written in. It is **one walk, not `ReferenceIndex`**: the index reads
  chapters from the *top-level* blocks, and a `\periph` division runs to the
  next `\periph` or `\id` (ticket 27) and so holds everything after it, which
  dropped every chapter of a front-matter book from the outline; and the
  outline needs the block containers, which the index does not model. A node
  with an empty span (`SPAN`, synthesized) is left out.
  **`textDocument/completion`** (`completion.rs`) answers after a `\` — the
  trigger character, and the same answer when invoked by hand part-way
  through a marker name — with the markers of the **document's own**
  stylesheet, so a project `custom.sty` completes. The `TextEdit` starts
  *after* the `\`, so the typed backslash is never doubled; a character
  style, a note and a milestone are `InsertTextFormat::SNIPPET` with their
  closing marker (`nd $1\nd*$0`, `f + $1\f*$0`, `qt-s$1\*$0`) and a paragraph
  marker is a plain Keyword. The filter is **ticket 20's rule**, now
  `usfm_semantic::placement::check(sheet, rule, parent_marker) -> Placement`
  (`Listed` / `NotListed` / `NotAllowed`, with `is_allowed()`): the crate's
  own `placement_verdict` is two lines over it, so the check that reports a
  misplaced marker and the completion that declines to offer one are one
  implementation and `usfm_semantic` still depends on nothing of the
  server's. Only the Error half filters — `\fq` is not offered outside a
  note; `marker-not-listed-here` is advisory and stays in the list. The
  parent comes from `locate`'s path by `placement_parent`'s three rules (a
  table cell and the inside of a character style filter nothing, a note is
  the parent of what it holds, otherwise the paragraph), and in practice it
  is nearly always a paragraph: the parser opens an implicit `\p` for
  anything written outside one, so even a `\` between blocks has one.
  **`textDocument/codeAction`** (`actions.rs`) offers one `quickfix` per
  diagnostic in the request's `context.diagnostics` — matched back to the
  parse's own by code *and* range, so the edit is computed from the
  toolchain's span and never from a protocol position — for the five codes
  whose fix is the repair written into the file: `unknown-marker` (delete the
  marker and the spaces after it), `character-style-not-closed` (insert the
  closer), `missing-note-caller` (insert ` +`),
  `attribute-value-not-quoted` (quote the value),
  `empty-milestone-attribute-list` (delete the `|` and the space before it).
  Every other code has none: `missing-id` would invent a book code,
  `verse-out-of-order` a renumbering. Each fix has a test that applies it,
  re-parses, and requires the code to be reported once less with no new code
  at all. Two spans do not say enough on their own and the tree finishes the
  job: `character-style-not-closed` points at the *opening* marker, and the
  `Char` node's span runs to wherever the style was closed — past the outer
  `\em*` in `\em a \+nd b\em*` — so the closer goes at the end of the node's
  **last child**, with the line break the text run swept up trimmed off; the
  closer itself is spelled from the source (`\+nd` closes with `\+nd*`)
- **Formatting and hover in the language server (ticket 31, M6).** Two
  capabilities, each a pure function with its own module and the wiring in
  `main.rs`. `textDocument/formatting` is `usfm_codegen`'s text of the stored
  document's parse — `apps/usfm_language_server/src/format.rs`'s `formatted`,
  the same trailing newline as `apps/usfm_cli/src/format.rs`, so
  `formatOnSave` and `usfm format --write` write the same bytes — returned as
  **one** `TextEdit` from `0:0` to the end of the file (a minimal diff is a
  follow-up if a client flickers). It **refuses** with `Ok(None)`, the
  protocol's `null`, plus a `window/showMessage` warning naming the first
  error, when the parse reported an Error: that is `usfm format --write`'s
  rule without `--force`, and `null` rather than `[]` because `[]` means
  "already formatted". An unchanged document does answer `[]`.
  `textDocument/hover` locates the innermost node whose **span** holds the
  cursor's byte offset (`locate.rs`: `locate` -> `Location { node, book,
  chapter, verse }`, one walk with `NodeRef::descendants`, which ticket 32
  reuses for symbols and code actions) and answers in markdown: for a styled
  node — `Para`, `Char`, `Note`, `Milestone`, `Sidebar`, `Periph`, and a
  `TableCell`, whose marker is spelled from `header`/`alignment`/`column` as
  the writer spells it — the **document's own** sheet's `\Name`,
  `\Description` and `\OccursUnder` list; for a `\c`/`\v`, and for text
  inside a verse, the reference (`GEN 1:1`). The hover range is the located
  node's whole span, not the marker's: the innermost node at an offset is
  already the smallest thing there, and this way the empty places (the space
  after `\p`) answer too. Text outside any chapter falls back to the style
  around it. **`StyleRule` had no `\Name` or `\Description`** — the `.sty`
  parser dropped both lines — so `usfm_style` now keeps them as
  `Option<String>`, `crates/usfm_parser/build.rs` writes them into the
  generated default sheet, and a derived rule (`\k-s`, an unknown milestone)
  has neither. `convert.rs` gained the inverse of `position` —
  `offset(source, Position)`, UTF-16, clamping a column past the line to its
  end and a line past the last to the end of the file — and
  `whole_document`. `tests/lsp.rs` now asserts both capabilities, the edit,
  its range, the refusal and its notification, and both shapes of hover
- **The language server, rebuilt (ticket 30, M6).**
  `apps/usfm_language_server` is a new workspace member on the `usfm` facade
  (`default-features = false`: the parser, the semantic pass and the
  diagnostics are all it needs), `tower-lsp-server` 0.23 and `tokio` — both
  workspace dependencies again. The binary is `usfm-language-server` and
  speaks LSP over stdio. It advertises exactly what it implements: full text
  synchronisation and UTF-16 positions, no hover, no formatting, no
  completion. `did_open`/`did_change` replace the document's text in a
  `HashMap<Uri, String>` behind a `tokio::sync::RwLock` and publish
  `usfm::parse_with`'s diagnostics — the union, so an editor shows the
  semantic checks too — with the kebab-case `Code` name as `code`, `"usfm"` as
  `source` and the severity mapped one to one. An empty list is published as
  readily as a full one, which is how the squiggles are cleared. Nothing is
  debounced: a book is milliseconds. Positions go through
  `usfm_span::LineIndex::line_col_utf16`, new beside `line_col` (the protocol
  counts a column in UTF-16 code units, so an emoji is two), and the
  conversion is `convert.rs`, a pure function of a `LineIndex` that tickets 31
  and 32 reuse. A project stylesheet — `initializationOptions.stylesheet`, or
  a `custom.sty` beside the open file — **extends** the default sheet the way
  `recovery.rs`'s `machine_py_custom_stylesheet` does (where
  `usfm parse --stylesheet` replaces it), is read once per path, and warns
  through `window/showMessage` once if it cannot be read rather than failing
  the parse. `tests/lsp.rs` spawns the built binary and speaks
  `Content-Length`-framed JSON-RPC by hand, with a reader thread and a timeout
  so a hang fails instead of blocking CI. In `vscode/`: `server:build:*` build
  `-p usfm_language_server`, `extension.ts` spawns the new binary name and its
  `LanguageClient` is constructed and started again (it had been commented
  out, so the extension shipped no server at all); the extension never had
  diagnostics of its own to retire. Not run under Miri, for the reason
  `scripts/miri.sh` now gives: tokio and a spawned process, and no byte
  handling of its own
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
    rest of the line; ticket 35 keeps the start, and since ticket 44 a
    division tracks verses and chapters itself — what is open before
    `\periph` ends before it, what the division opens ends inside it — where
    ticket 27 had suspended them across the whole division, leaving every
    `\c` and `\v` in one without its `eid`), and a default attribute value that
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
  right for exactly that reason; an `\xo` is the exception, since it takes no
  un-plussed child at all (ticket 36), so its closer goes before a closed
  `\xt` too — ticket 41). The parser reports nothing for an implicit
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
  `usfm parse <files> --from usfm|usx --format usx|html|json|usfm|sile|prompt`, `--stylesheet`,
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
crate split, M4 `usfm_semantic`, M5 `usfm_codegen`, M6 language server, M7
the USX reader. All seven are closed, each with its exit criteria recorded in
the spec (M5 on 2026-09-20, tickets 25–27 and 34–36). Ticket 37 paid back the M5 parse regression, and **M6's agent
tickets are done**: the language server is rebuilt in
`apps/usfm_language_server` with diagnostics (ticket 30), formatting and
hover (31) and symbols, completion and code actions (32), and **M6 is
closed** (2026-09-26): ticket 33 deleted the old server and its SQLite
lexicon crate, and the lexicon feature was decided against. M6's other exit
criteria were checked and recorded in the spec on 2026-09-20 (the boundary
benchmark rerun is `docs/benchmarks.md`, "M6 close": within noise). Tickets
10 and 33, the two that waited on Michael, are both answered and done
(2026-09-26), so nothing is blocked on him. **M7, the USX reader** (ticket 42, specified 2026-09-26; tickets 45–49), is
**closed** (2026-09-27): `usfm_usx::read_usx` on `roxmltree`, `usfm::parse_usx`
and `usfm parse --from usx`, gated by USX -> `Document` -> USX and USX ->
USFM -> USX over both conformance roots, fuzzed, under Miri and benchmarked;
the exit criteria are recorded in the spec. **No milestone is open.** What
is left — tickets 39 and 40 (`needs-triage`: the server's per-request parse,
formatting as a minimal diff), the hardening plan's unchecked boxes, and the
output formats with no plan yet — is listed in the spec under "After M6:
what is left"; the next milestone is written from there.
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
│   ├── usfm_style/        # The stylesheet: StyleSheet, StyleRule, and the
│   │                      #   generated DEFAULT_STYLESHEET (`usfm.sty`)
│   ├── usfm_ast/          # AST nodes, Visit / VisitMut / Fold, Cursor, PlainText
│   ├── usfm_diagnostics/  # Diagnostic, Code, Severity, ParseResult, rendering
│   ├── usfm_parser/       # Lexer + parser. A library, no binary
│   ├── usfm_fix/          # Fixes for diagnostics: fix(&Document, source,
│   │                      #   &Diagnostic) -> Option<Fix>, and apply. Does
│   │                      #   not parse; `usfm fix` and the server call it
│   ├── usfm_semantic/     # Checks over a finished tree: analyze(&Document)
│   │                      #   -> Vec<Diagnostic>. Reports, never repairs.
│   │                      #   Also ReferenceIndex, the chapter/verse index,
│   │                      #   and `placement::check`, the `OccursUnder` rule
│   │                      #   the checks and the server's completion share,
│   │                      #   and `citation`, references written as text
│   ├── usfm_usx/          # AST <-> USX: the XML tree and its writer, and
│   │                      #   `read_usx` / `read_usx_with` (ticket 45), USX
│   │                      #   into a `Document` + diagnostics on `roxmltree`;
│   │                      #   USX 2 reads to the parser's verse ends (47)
│   ├── usfm_html/         # AST -> HTML: ToHtml, SerializeHtml, Context
│   ├── usfm_json/         # AST -> JSON: the tree as it is, one object per node
│   ├── usfm_paratext/     # A Paratext project folder: Settings.xml,
│   │                      #   BookNames.xml, book files, its stylesheet,
│   │                      #   and its interlinear glosses and Lexicon.xml
│   ├── usfm_codegen/      # AST -> USFM: one canonical spelling per construct,
│   │                      #   `to_usfm_string`. Round-trips the tcdocs corpus
│   ├── usfm_pipeline/     # Document -> Document/text: replacements, sections,
│   │                      #   diglot, prompt, SILE, the format dispatch
│   └── usfm/              # Facade: re-exports the above behind features, and
│                          #   `usfm::parse` / `parse_with` / `parse_with_options`,
│                          #   whose diagnostics are the parser's plus
│                          #   usfm_semantic's, and `parse_usx` /
│                          #   `parse_usx_with`, the reader's plus the same.
│                          #   `apps/` and `tasks/` depend on this, not on
│                          #   the pieces
├── apps/
│   ├── usfm_cli/          # The `usfm` binary: clap, watch mode, diagnostics;
│   │                      #   `usfm parse --from usx` reads USX (ticket 48);
│   │                      #   `usfm format`; `usfm fix` (dry run by default)
│   └── usfm_language_server/ # The `usfm-language-server` binary (M6): LSP
│                          #   over stdio on tower-lsp-server + tokio. Keeps
│                          #   the open text, publishes `usfm::parse_with`'s
│                          #   diagnostics, and answers formatting, hover,
│                          #   symbols, completion and code actions.
│                          #   `vscode/` spawns it
├── tasks/
│   ├── conformance/       # tcdocs + usfm-grammar runner (the `usfm_tests` crate),
│   │                      #   its fixtures, patches, baseline and the
│   │                      #   round-trip property (`roundtrip.rs`) all three
│   │                      #   round-trip checks share, and M7's two USX
│   │                      #   properties (`usx_properties.rs`)
│   ├── benchmark/         # criterion benches over a committed corpus
│   └── fuzz/              # cargo-fuzz targets (own workspace, nightly)
└── tcdocs/                # Git submodule: official USFM test suite
```

Every crate the ADR's tree calls for now exists: `crates/usfm_semantic`
arrived with ticket 19 and `crates/usfm_codegen` with ticket 25. M6 rebuilt
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

# M7's USX properties over every compared case, gated the same way (ticket 46):
# reference -> read_usx -> USX, and reference -> read_usx -> USFM -> parse -> USX
cargo run --package usfm_tests -- --usx-read tasks/conformance/usx-read-known.txt
cargo run --package usfm_tests -- --usx-roundtrip tasks/conformance/usx-roundtrip-known.txt
cargo run --package usfm_tests -- --write-usx-read-known tasks/conformance/usx-read-known.txt
cargo run --package usfm_tests -- --write-usx-roundtrip-known tasks/conformance/usx-roundtrip-known.txt

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
- A Cargo workspace: fourteen `crates/`, two `apps/`, and the `tasks/` packages (`tasks/fuzz` has its own workspace)
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
