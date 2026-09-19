# usfm-tools

Rust-based USFM parser with AST, language server, and multiple output formats.

## What "Progress" Means

This project has three parallel work streams, all in progress:

1. **Parser Development** (`crates/usfm_parser/`, `crates/usfm_ast/`)
   - Parse USFM into AST structure
   - Handle edge cases and malformed input gracefully
   - Expand grammar coverage

2. **Language Server** (`wip/usfm_language_server/`) — parked
   - Power VS Code extension for editing USFM
   - Validation, diagnostics, formatting
   - Eventually: completion, hover, go-to-definition
   - Parked outside the workspace in `wip/` (ticket 01) and does not build;
     M6 rebuilds it in `apps/` on `ParseResult`. Do not work this stream until then.

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
  `usfm_semantic`'s `checks`, parser lib),
  gates lint with
  `cargo clippy --workspace --all-targets -- -D warnings` (in `scripts/gate.sh`
  since 2026-09-19, ticket 02: the workspace is clippy-clean, so a new warning
  fails the build) and
  gates tcdocs on `tasks/conformance/tcdocs-baseline.txt`, the list of known failures
  (currently empty). It fails on a regression *and* on a stale entry, so when
  you fix a tcdocs case, remove it from the baseline (or regenerate with
  `--write-baseline`) in the same commit. Last in the gate is `scripts/miri.sh`
  (ticket 05): `cargo +nightly miri test` over the `usfm_span`, `usfm_ast` and
  `usfm_diagnostics` libs, `usfm_usx`'s lib, `usfm_html`'s `escape` tests, the
  parser lib
  and the `whitespace`, `attributes`, `usx_text`, `verse_ends`, `spans` and
  (11 of 92) `recovery` suites, about 3 min 50 s. It needs a nightly toolchain
  with `miri` and `rust-src`, which CI installs in its own step and
  `scripts/session-start.sh` installs on a fresh VM; `rust-toolchain.toml`
  stays pinned at 1.98.0 for everything else.
- Fuzzing is on demand, not in the gate or CI (ticket 06): `cargo +nightly fuzz
  run --fuzz-dir tasks/fuzz parse_lossy -- -max_total_time=600 -max_len=65536`,
  and the same for `parse_utf8` and `parse_html`. The first two assert no panic,
  the span invariants (`usfm_parser::span_check`, shared with `crates/usfm_parser/tests/spans.rs`
  behind the `testing` feature) and that the USX output is well-formed XML;
  `parse_html` (ticket 14) asserts no panic and that the HTML output is balanced
  and properly escaped, checked by a scanner in `tasks/fuzz/src/lib.rs` rather
  than by an HTML parser. `tasks/fuzz` is
  outside the workspace, so run its clippy separately; see `tasks/fuzz/README.md`.

**Traversal API (Phase 3, complete 2026-09-12)** lives in `usfm_ast`:
`visit::Visit` / `visit_mut::VisitMut` (generated from one macro, same method
names, no context parameter: hold `document.style_sheet()` if you resolve
styles), `fold::Fold` (bottom-up, one associated type per node kind),
`document.reference_index()` (`ReferenceIndex`: chapters, verses, their
milestone paths, `verse(c, v)`, `VerseRef::nodes()` / `text()`), and
`text::PlainText`. The parser re-exports the visitor modules. `Cursor` stays
read-only.

Recent progress:
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
  `usfm parse <files> --format usx|html|json|sile|prompt`, `--stylesheet`,
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
  out-of-order cell is `unexpected-table-column`. A verse that starts inside a
  non-verse-text paragraph (`\lit`) ends the previous verse before it
- Attribute validation: `empty-attribute-list`, `no-default-attribute`,
  `default-attribute-with-others`, `attribute-value-not-quoted`,
  `missing-attribute-value`; and `number-has-leading-zero` for `\c 091`/`\v 01`.
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
  stylesheet entry allows `NEST` *and* its own closing marker lies ahead in the
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
crate split, M4 `usfm_semantic`, M5 `usfm_codegen`, M6 language server. M1, M2
and M3 all closed 2026-09-19 (exit criteria recorded in the spec); **M4
(`usfm_semantic`) is next**: move the non-syntactic checks (`OccursUnder`
placement, table columns, leading zeros, the attribute validation that needs
the stylesheet) and `ReferenceIndex` out of the parser, with `usfm::parse()`
still returning the union so tcdocs is unchanged. M4 is ticketed (18–23). Tickets are in
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
│   │                      #   -> Vec<Diagnostic>. Reports, never repairs
│   ├── usfm_usx/          # AST -> USX: the XML tree, its writer and reader
│   ├── usfm_html/         # AST -> HTML: ToHtml, SerializeHtml, Context
│   ├── usfm_json/         # AST -> JSON: the tree as it is, one object per node
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
│   │                      #   its fixtures, patches and baseline
│   ├── benchmark/         # criterion benches over a committed corpus
│   └── fuzz/              # cargo-fuzz targets (own workspace, nightly)
├── tcdocs/                # Git submodule: official USFM test suite
└── wip/                   # Outside the workspace, parked until M6, does not build
    ├── usfm_language_server/  # LSP server (parked)
    └── data_layer/            # Data persistence (parked)
```

`crates/usfm_codegen` (M5) is the one the ADR's tree still lacks;
`crates/usfm_semantic` arrived with ticket 19 and M6 rebuilds the language
server under `apps/`.

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
