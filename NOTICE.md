# Third-party material

The code in this repository is MIT licensed (see `LICENSE`). It also contains or
derives from the following, which keep their own terms.

- **USFM/USX test suite** (`tcdocs/`, a git submodule of
  https://github.com/usfm-bible/tcdocs). Copyright © 2022-2023 the members of the
  USFM/USX technical committee. Specifications, documentation, schemas and data
  are CC BY 4.0; code is MIT. The AST snapshots in `usfm_parser/tests/snapshots/`
  named `tcdocs__*` and the diffs in `tests/tcdocs-patches/` quote its test inputs
  and are derived from it under CC BY 4.0. Scripture excerpts inside those inputs
  belong to their respective publishers.
- **`usfm_parser/usfm.sty`**: the default USFM stylesheet distributed with
  Paratext, by United Bible Societies and SIL International, unmodified apart
  from what its own header records. Documentation: https://ubsicap.github.io/usfm/
- **Lexer `Source` and `Span` design** (`usfm_parser/src/lexer/source.rs`,
  `usfm_ast/src/span.rs`): adapted from oxc (https://github.com/oxc-project/oxc),
  MIT, Copyright (c) 2023-present VoidZero Inc. & Contributors.

The USFM reference documentation is not redistributed here. Run
`python3 .claude/import_docs.py` to fetch a local copy into `.claude/docs/`.
