# 0001. Crate layout follows oxc; its memory model does not

Date: 2026-09-19
Status: accepted
Status: implemented (M3, 2026-09-19) — the tree below exists apart from
`usfm_semantic` (M4), `usfm_codegen` (M5) and the language server (M6);
see `.scratch/oxc-layout/spec.md` for the M3 exit record.

## Context

`usfm_parser` holds the lexer, the parser, the CLI, two serializers, an XML tree
and the text replacements. Phase 4 of `docs/plans/hardening.md` already splits the
outputs into crates. oxc is the reference for how a Rust parser project is
organised, and parts of this repo already come from it (`u32` spans, the lexer's
`Source`, generated visitors).

## Decision

Adopt oxc's organisation: one job per crate, a parser that reports syntax only, a
separate semantic pass, and `tasks/` for conformance, benchmarks and fuzzing.

Target layout:

```
crates/
  usfm_span          Span, line/column lookup
  usfm_style         stylesheet (exists)
  usfm_ast           nodes, Visit / VisitMut / Fold, Cursor, PlainText (exists)
  usfm_diagnostics   Diagnostic, Code, Severity, ParseResult policy, rendering
  usfm_parser        lexer + parser only; library, no binary
  usfm_semantic      ReferenceIndex, placement and numbering checks
  usfm_codegen       AST -> USFM
  usfm_usx           AST -> USX (owns the XML tree and writer)
  usfm_html          AST -> HTML
  usfm_json          AST -> JSON
  usfm_pipeline      text replacements, diglot weaving, sectioning, prompt format
  usfm               facade: re-exports the above behind features
apps/
  usfm_cli           clap, watch mode, diagnostics printing
  usfm_language_server   (rebuilt in Phase 5)
tasks/
  conformance        tcdocs runner (was `tests/`, moved by ticket 17)
  benchmark          criterion benches over a fixed corpus
  fuzz               cargo-fuzz targets
wip/                 outside the workspace: old language server, data_layer
```

Dependencies point one way: span <- style, ast <- diagnostics <- parser <-
semantic <- outputs <- pipeline <- facade <- apps. No output crate depends on
another output crate.

Not adopted:

- **Arena allocation** (`oxc_allocator`, `Box<'a>`, `Vec<'a>`). USFM trees are
  shallow and most bytes are `Text` borrowed from the source. An arena would add a
  lifetime to every consumer and fight `into_owned()`, `VisitMut` and `Fold`, which
  the output pipelines are built on. Reopen only if the benchmarks show allocation
  dominating parse time.
- **`Atom` / `CompactStr`, node ids, `oxc_traverse`.** `StyleId` interns the
  strings that repeat; `Cursor` and `ReferenceIndex` cover ancestry.
- **`ast_tools` codegen.** The visitor macro is enough for ~20 node kinds. Reopen
  if `fold.rs`, `into_owned.rs` and the visitors drift apart.
- **Further lexer micro-optimisation** without a benchmark that asks for it.

## Consequences

- Performance work needs a number first: the benchmark harness lands before the
  split, so the split itself is measured.
- The `unsafe` borrowed from oxc's lexer is kept only if it passes Miri and the
  benchmark shows it earns its place; otherwise it is replaced with safe code.
- Moving checks from the parser to `usfm_semantic` changes which call reports
  which `Code`. `ParseResult` keeps reporting all of them through the facade, so
  the tcdocs harness and `strict()` see no difference.
- Crates move under `crates/`, so paths in CLAUDE.md, CI and the plan change once.
