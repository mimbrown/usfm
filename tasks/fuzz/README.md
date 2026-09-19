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

Both call `usfm_fuzz::check_source` (`src/lib.rs`), which asserts what holds for
*any* input, since the parser never fails:

1. parsing does not panic;
2. every span satisfies the invariants in `usfm_parser::span_check` — in
   bounds, not inverted, on character boundaries, and starting at the marker
   the node was read from (or, for a node the parser opened to recover, at or
   before the content it holds). `usfm_parser/tests/spans.rs` asserts the same
   invariants over hand-written inputs, so the two cannot drift apart;
3. `to_usx_string` does not panic and produces well-formed XML, checked by
   reading it back with `xml-rs` to the end of the document.

| Target | Input |
| --- | --- |
| `parse_lossy` | any bytes, through `String::from_utf8_lossy` |
| `parse_utf8` | any bytes, skipped unless they are valid UTF-8 |

## Running

```bash
# From the repository root. 10 minutes is the milestone's exit criterion.
cargo +nightly fuzz run --fuzz-dir tasks/fuzz parse_lossy -- -max_total_time=600 -max_len=65536
cargo +nightly fuzz run --fuzz-dir tasks/fuzz parse_utf8  -- -max_total_time=600 -max_len=65536

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

Then write the minimised input as a test — `usfm_parser/tests/recovery.rs` for a
parser rule, `tests/spans.rs` for a span invariant, `tests/usx_text.rs` for the
USX writer — and fix it. A finding is a bug: it is not worked around by widening
an invariant or catching the panic. A finding that is not fixed in the same
sitting goes to `findings/` with a note, so it is not lost.

## Seeds

`./seed.sh` copies the conformance inputs into `corpus/parse_lossy/` and
`corpus/parse_utf8/`, named after the test they came from:

- the tcdocs inputs (`tcdocs/tests/*/*/origin.usfm`), as `<category>__<case>.usfm`;
- the vendored usfm-grammar fixtures
  (`tests/fixtures/usfm-grammar/{autofix/*,bugfixes/*/origin.usfm}`), as
  `usfm-grammar__<dir>__<name>.usfm`. The `autofix` inputs are deliberately
  malformed, which is what makes them worth seeding.
- the vendored machine.py fixtures (`tests/fixtures/machine-py/*/*.SFM`), as
  `machine-py__<project>__<book>.usfm`. Among them are a zero-byte book and
  two whose `\id` disagrees with their filename.

It is idempotent, and it truncates a seed longer than `-max_len` to the last
whole line that fits, which is what libFuzzer would do with it anyway. Run
`git submodule update --init tcdocs` first.

The seeds are committed. libFuzzer also *writes* to the corpus directory as it
finds new coverage, so after a run `corpus/` holds more than the seeds;
`./seed.sh --prune` removes everything that is not a seed again.

## Results

10 minutes per target on the final code (4 vCPUs, one worker each, the two
targets run side by side, `-max_len=65536`), starting from the committed seeds.
exec/s is low for a fuzzer because the corpus carries whole books: one input is
up to 64 KiB of USFM, parsed and serialized. `parse_lossy` is the slower of the
two because every input reaches the parser, while `parse_utf8` skips the ones
that are not UTF-8.

| Target | exec/s | corpus at the end | cov | ft | crashes |
| --- | --- | --- | --- | --- | --- |
| `parse_lossy` | 49 (29 749 runs in 601 s) | 1647 files, 14.9 MB | 4175 | 22 337 | none |
| `parse_utf8` | 83 (50 097 runs in 601 s) | 1622 files, 13.7 MB | 4200 | 22 225 | none |

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
