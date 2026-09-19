# usfm-tools

Rust-based USFM parser with AST, language server, and multiple output formats.

## What "Progress" Means

This project has three parallel work streams, all in progress:

1. **Parser Development** (`usfm_parser/`, `usfm_ast/`)
   - Parse USFM into AST structure
   - Handle edge cases and malformed input gracefully
   - Expand grammar coverage

2. **Language Server** (`usfm_language_server/`)
   - Power VS Code extension for editing USFM
   - Validation, diagnostics, formatting
   - Eventually: completion, hover, go-to-definition

3. **Output Generation** (`usfm_style/`, future crates)
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
pipelines. Every recovery rule is a `Code` variant with a test in
`usfm_parser/tests/recovery.rs`.

tcdocs status (260 tests, 2026-09-12):
- 215 passed, 0 failed, 0 panicked, 1 skipped, 44 expected failures, 0 unexpected passes
- Harness semantics: `pass` inputs must match USX with no error diagnostics;
  `fail` inputs must either report an error or match the expected USX.
- Ten reference files are read through a patch in `tests/tcdocs-patches/`
  (rules in its README): a unified diff with the rationale above it, for a
  reference quirk or an accepted deviation, never for a parser gap. The
  harness fails a patch that stops applying or that the parser no longer
  needs. `cargo run -p usfm_tests -- --show <name>` prints one test's
  diagnostics, output and patched expected USX.
- AST snapshot corpus: `cargo test -p usfm_parser --test snapshot` (review with `cargo insta review`)
- Run `git submodule update --init tcdocs` first. Without it the `usfm_tests` build
  fails on purpose, so a zero-test run can never report a pass rate.
- `cargo build --workspace` still fails on `usfm_language_server`; use
  `--exclude usfm_language_server`.
- CI (`.github/workflows/ci.yml`) runs the unit and integration suites
  (`recovery`, `snapshot`, `spans`, `whitespace`, `attributes`, `verse_ends`, `usx_text`,
  parser lib) and
  gates tcdocs on `tests/tcdocs-baseline.txt`, the list of known failures
  (currently empty). It fails on a regression *and* on a stale entry, so when
  you fix a tcdocs case, remove it from the baseline (or regenerate with
  `--write-baseline`) in the same commit.

**Traversal API (Phase 3, complete 2026-09-12)** lives in `usfm_ast`:
`visit::Visit` / `visit_mut::VisitMut` (generated from one macro, same method
names, no context parameter: hold `document.style_sheet()` if you resolve
styles), `fold::Fold` (bottom-up, one associated type per node kind),
`document.reference_index()` (`ReferenceIndex`: chapters, verses, their
milestone paths, `verse(c, v)`, `VerseRef::nodes()` / `text()`), and
`text::PlainText`. The parser re-exports the visitor modules. `Cursor` stays
read-only.

Recent progress:
- The CLI writes USX through `usx.rs` (`usx::to_usx_string`; `serialize_usx.rs` is
  gone), so it keeps word-level attributes. The `XmlNode` writer escapes text and
  adds no whitespace inside mixed content; `usfm_parser/tests/usx_text.rs`
- Phase 3 traversal API (above); the old `usfm_parser::visit_mut::Context`
  is gone, `TextReplacement::apply_to(&mut document)` replaces
  `visit_mut_document`
- Verse and chapter ends are emitted by the parser as it goes (`OpenVerse` in
  `parser.rs`, plan D4) instead of by a path-arithmetic pass after parsing;
  the placement rules are one test each in `usfm_parser/tests/verse_ends.rs`
- `Inline::OptBreak` for `//` (USX `<optbreak/>`, HTML `<wbr>`); the default
  attribute value is verbatim, quotes included, as Paratext reads it; a verse
  that starts in a non-verse-text paragraph (`\lit`) ends inside it when the
  previous verse started there too
- `StyleRule::occurs_under` carries the stylesheet's `OccursUnder` list. A
  note-only marker outside its note (`\xq` in a paragraph) is
  `marker-not-allowed-here` (Error); any other unlisted placement (`\f` under
  `\cl`, which Paratext accepts) is `marker-not-listed-here` (Info)
- `TableCell::column` keeps the source column (`\th3` stays 3); a gap or
  out-of-order cell is `unexpected-table-column`. A verse that starts inside a
  non-verse-text paragraph (`\lit`) ends the previous verse before it
- Attribute validation: `empty-attribute-list`, `no-default-attribute`,
  `default-attribute-with-others`, `attribute-value-not-quoted`,
  `missing-attribute-value`; and `number-has-leading-zero` for `\c 091`/`\v 01`.
  Seven `validated=fail` inputs now report an error instead of passing silently
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
  `usfm_parser/tests/whitespace.rs`): only ASCII whitespace is normalised,
  `~` is a no-break space, trailing whitespace is dropped only at
  paragraph-level boundaries. The tcdocs harness ignores whitespace at note
  and cell ends on both sides because the reference files disagree there.
- `\usfm 3.1` sets `<usx version>`; the marker stays in the AST as a paragraph
  (`Document::usfm_version()`) and the USX serializers skip it
- Phase 1 complete: D1, D2 (spans), D3 (owned stylesheet), D5 (`into_owned`),
  D6 (attributes as a field, `Chunk` removed)
- Derived (`\k-s`) and unknown (`\zaln-s`, `\ts`) milestones are registered and kept
  rather than dropped; `Block::Milestone` for milestones between blocks
- `tests/spans.rs` checks span invariants mechanically; snapshots render spans
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
  carry `SPAN`.
- D6: `Char::attributes` is a field; `Inline::Attributes` and `Chunk` are gone.
- D5: `Document::into_owned()` detaches a parse from its source.

Priority areas, next: the route is `.scratch/oxc-layout/spec.md` (decision in
`docs/adr/0001-oxc-style-crate-layout.md`): oxc's crate organisation, not its arena.
Milestones in order: M1 workspace builds clean, M2 benchmarks + Miri + fuzz, M3
crate split, M4 `usfm_semantic`, M5 `usfm_codegen`, M6 language server. Tickets
are in `.scratch/oxc-layout/issues/`, written one milestone ahead. Unattended
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

```
usfm-tools/
├── usfm_ast/              # AST node definitions
├── usfm_parser/           # Parser implementation
├── usfm_language_server/  # LSP server
├── usfm_style/            # Styling/output
├── data_layer/            # Data persistence
├── tests/                 # Integration tests (usfm_tests crate)
├── tcdocs/                # Git submodule: official USFM test suite
└── build.rs               # Build configuration
```

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
cargo run --package usfm_tests -- --baseline tests/tcdocs-baseline.txt

# Regenerate the list after fixing or regressing cases
cargo run --package usfm_tests -- --write-baseline tests/tcdocs-baseline.txt

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
