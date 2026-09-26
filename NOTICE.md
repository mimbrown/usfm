# Third-party material

The code in this repository is MIT licensed (see `LICENSE`). It also contains or
derives from the following, which keep their own terms.

- **USFM/USX test suite** (`tcdocs/`, a git submodule of
  https://github.com/usfm-bible/tcdocs). Copyright © 2022-2023 the members of the
  USFM/USX technical committee. Specifications, documentation, schemas and data
  are CC BY 4.0; code is MIT. The AST snapshots in `crates/usfm_parser/tests/snapshots/`
  named `tcdocs__*` and the diffs in `tasks/conformance/tcdocs-patches/` quote its test inputs
  and are derived from it under CC BY 4.0, as are the fuzz seeds in
  `tasks/fuzz/corpus/` named after its test directories (`advanced__*`,
  `basic__*`, `biblica__*`, `introductions__*`, `mandatory__*`,
  `paratextTests__*`, `samples-from-wild__*`, `specExamples__*`,
  `special-cases__*`, `usfmjsTests__*`), which are copies of its test inputs.
  The two inputs that quote the New International Version
  (`biblica/PublishingVersesNotClosed` and `…WithFormatting`) are Biblica's
  copyright rather than open data, and neither is copied into this repository.
  Scripture excerpts inside those inputs belong to their respective publishers,
  and any licence notice an input carries (a `\rem` line) is kept in the copy.
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
- **unfoldingWord® Literal Text and Greek New Testament, Acts**
  (`tasks/benchmark/corpus/aligned/`, from the test resources of
  https://github.com/unfoldingWord/usfm-js at commit
  `0ecae6f169f912e1c30da6f519a7724d31dcd841`, fetched 2026-09-26). The
  scripture text is unfoldingWord's ULT and UGNT, **CC BY-SA 4.0**; attribution:
  unfoldingWord, https://www.unfoldingword.org/ult and
  https://www.unfoldingword.org/ugnt. The licence's legal code is copied
  verbatim to `tasks/benchmark/corpus/aligned/LICENSE`. usfm-js itself is ISC
  (its `package.json`); none of its code is vendored. The two files are
  adapted: `tasks/benchmark/corpus/tools/usfmjs_oldformat.py` closes the
  alignment and key-term milestones with `\*`, and changes nothing else (the
  directory's README has the details). They are under CC BY-SA 4.0 as
  adapted, and so are the fuzz seeds named `usfm-js__*` in
  `tasks/fuzz/corpus/`, which are copies of them truncated to 64 KiB.
- **`crates/usfm_parser/usfm.sty`**: Paratext's default stylesheet for
  combined study Bible projects (`usfm_sb.sty`, version 3.0.11), by United
  Bible Societies and SIL International, byte for byte the copy tcdocs carries
  at `grammar/usfm_sb.sty`, and so under CC BY 4.0 with the rest of tcdocs'
  data (above). It is compiled into `usfm_parser` together with
  `crates/usfm_parser/usfm-extra.sty`, this project's own additions and
  corrections. Documentation: https://ubsicap.github.io/usfm/
- **Lexer `Source` and `Span` design** (`crates/usfm_parser/src/lexer/source.rs`,
  `crates/usfm_span/src/span.rs`): adapted from oxc (https://github.com/oxc-project/oxc),
  MIT, Copyright (c) 2023-present VoidZero Inc. & Contributors. Its licence
  text follows.

  > The MIT License (MIT)
  >
  > Copyright (c) 2023-present VoidZero Inc. & Contributors
  >
  > Permission is hereby granted, free of charge, to any person obtaining a copy
  > of this software and associated documentation files (the "Software"), to deal
  > in the Software without restriction, including without limitation the rights
  > to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
  > copies of the Software, and to permit persons to whom the Software is
  > furnished to do so, subject to the following conditions:
  >
  > The above copyright notice and this permission notice shall be included in all
  > copies or substantial portions of the Software.
  >
  > THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
  > IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
  > FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
  > AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
  > LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
  > OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
  > SOFTWARE.

The USFM reference documentation is not redistributed here. Run
`python3 .claude/import_docs.py` to fetch a local copy into `.claude/docs/`.
