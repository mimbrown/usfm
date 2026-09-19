# Third-party material

The code in this repository is MIT licensed (see `LICENSE`). It also contains or
derives from the following, which keep their own terms.

- **USFM/USX test suite** (`tcdocs/`, a git submodule of
  https://github.com/usfm-bible/tcdocs). Copyright © 2022-2023 the members of the
  USFM/USX technical committee. Specifications, documentation, schemas and data
  are CC BY 4.0; code is MIT. The AST snapshots in `crates/usfm_parser/tests/snapshots/`
  named `tcdocs__*` and the diffs in `tasks/conformance/tcdocs-patches/` quote its test inputs
  and are derived from it under CC BY 4.0. Scripture excerpts inside those inputs
  belong to their respective publishers.
- **usfm-grammar test fixtures** (`tasks/conformance/fixtures/usfm-grammar/`, from
  https://github.com/Bridgeconn/usfm-grammar at commit
  `4ee1b91c9b725f7f5be3654805ab6447b111da58`, fetched 2026-09-19). Copyright ©
  2021 Bridge Connectivity Solutions, MIT; the licence text is copied verbatim
  to `tasks/conformance/fixtures/usfm-grammar/LICENSE`. Only its `tests/autofix/` and
  `tests/bugfixes/` directories are vendored. The fuzz seeds named
  `usfm-grammar__*` in `tasks/fuzz/corpus/` are copies of those inputs.
  Scripture excerpts inside those inputs belong to their respective publishers.
- **machine.py test data** (`tasks/conformance/fixtures/machine-py/`, from
  https://github.com/sillsdev/machine.py at commit
  `e2af2c868043c2b3594789d1110b05eda96de30e`, fetched 2026-09-19). Copyright ©
  2022 SIL International, MIT; the licence text is copied verbatim to
  `tasks/conformance/fixtures/machine-py/LICENSE`. Only its `tests/testutils/data/usfm/`
  directory is vendored. The fuzz seeds named `machine-py__*` in
  `tasks/fuzz/corpus/` are copies of those inputs. Scripture excerpts inside
  those inputs belong to their respective publishers.
- **World English Bible** (`tasks/benchmark/corpus/web/`), the benchmark
  corpus. The WEB is in the **public domain**: no copyright, no attribution
  requirement, no restriction on redistribution. It is eBible.org's edition,
  converted from the USFX file `eng-web.usfx.xml` in
  https://github.com/seven1m/open-bibles (commit `f257a35`, fetched
  2026-09-19) by `tasks/benchmark/corpus/tools/usfx_to_usfm.py`. The files
  under `tasks/benchmark/corpus/synthetic/` are derived from it by
  `tasks/benchmark/corpus/tools/synthesize.py`: the scripture text is the
  same public-domain WEB, while the word attributes, alignment milestones,
  lemmas, Strong's numbers and morphology in them are machine-generated
  filler for benchmarking and carry no linguistic claim.
- **`crates/usfm_parser/usfm.sty`**: the default USFM stylesheet distributed with
  Paratext, by United Bible Societies and SIL International, unmodified apart
  from what its own header records. Documentation: https://ubsicap.github.io/usfm/
- **Lexer `Source` and `Span` design** (`crates/usfm_parser/src/lexer/source.rs`,
  `crates/usfm_span/src/span.rs`): adapted from oxc (https://github.com/oxc-project/oxc),
  MIT, Copyright (c) 2023-present VoidZero Inc. & Contributors.

The USFM reference documentation is not redistributed here. Run
`python3 .claude/import_docs.py` to fetch a local copy into `.claude/docs/`.
