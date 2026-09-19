#!/usr/bin/env bash
# Miri over the suites that exercise the lexer, the parser's byte handling and
# `usfm_ast`'s `string_parser` (ticket 05; M2 exit in
# `.scratch/oxc-layout/spec.md`). Called from `scripts/gate.sh` after the tests.
#
# Miri needs nightly. `scripts/session-start.sh` installs it, and CI installs it
# in a step before the gate; `rust-toolchain.toml` stays pinned at 1.98.0 for
# everything else, which is why every command below says `+nightly`.
#
# Ticket 05 replaced 26 of the workspace's 27 `unsafe` uses (blocks and
# `unsafe fn`s) with safe code after measuring that they cost nothing. What is
# left is `ParserImpl::src` in `usfm_parser/src/cursor.rs`, one
# `str::get_unchecked` on the parser's hottest path, and checking it is this
# script's job: every span the parser slices a `&str` with passes through it.
#
# Miri interprets the program on one host thread, so the test harness's threads
# buy nothing and the wall time is the sum of the suites. Local wall time on the
# 4-vCPU Xeon @ 2.80GHz the agent loop runs on, with the Miri build already
# done: **about 3 min 50 s** (per-suite times in the comments below). The
# budget for the CI step is 5 minutes, so a suite added here has to earn it.
#
# Not run, on purpose:
# * `usfm_parser --test snapshot` — the insta corpus, hundreds of files of I/O
#   and no byte handling the suites below do not already reach.
# * `usfm_tests` (tcdocs) — 260 cases, hours under Miri.
# * `usfm_parser --lib text_replacements::*` — 66 s of the lib suite's 86 s, all
#   of it inside the `regex` crate, which is not this repo's code.
# * 65 of the 76 `recovery` tests — the 11 kept are the ones whose input drives
#   the lexer somewhere unusual (a lone `\`, an unterminated quote, an escaped
#   one, a marker name with `-` or `_`, a newline inside an attribute list, EOF
#   inside a character style, a malformed number through `string_parser`). The
#   rest re-lex ordinary text and cost about 6 s each, nearly all of it insta
#   reading its snapshot file under Miri.
set -euo pipefail
cd "$(dirname "$0")/.."

# Snapshots are read, never written, exactly as in `scripts/gate.sh`.
export INSTA_UPDATE=no
# insta finds the workspace root by shelling out to `cargo metadata`, and Miri
# cannot spawn a process. Telling it the root outright skips that.
export INSTA_WORKSPACE_ROOT="$PWD"

run() {
  echo "==> cargo +nightly miri test $*"
  cargo +nightly miri test "$@"
}

# `Span` and `LineIndex`: byte offsets sliced out of a `&str`.          ~1 s
run -p usfm_span --lib

# `Diagnostic::render` / `to_json_line` over a `LineIndex`, and the JSON
# escaping that walks a `&str` char by char.                            ~5 s
run -p usfm_diagnostics --lib

# `string_parser` (through `number`), `cursor`, the visitors, `reference`,
# `text`.                                                              ~9 s
run -p usfm_ast --lib

# `write_escaped`: a byte-table scan over a `&str` that slices at the indices
# it stops on, plus the reader round trip (ticket 13).                  ~3 s
run -p usfm_usx --lib

# `write_escaped` / `write_escaped_attribute`: the same byte-table scan over a
# `&str`, slicing at the indices it stops on (ticket 14).               ~1 s
# Only the `escape` tests: the rest of `usfm_html`'s lib suite parses whole
# documents through the HTML writer, which is 14 s of Miri for byte handling
# the parser suites below already cover (whole suite: 15 s).
run -p usfm_html --lib escape::

# The lexer's own unit tests, including `lexer::source`, plus the parser and
# style unit tests.                                                   ~20 s
run -p usfm_parser --lib -- --skip text_replacements

# Whole-document parses: the lexer over real markup, and the span
# invariants checked mechanically.
run -p usfm_parser --test whitespace   # ~17 s
run -p usfm_parser --test attributes   # ~13 s
run -p usfm_parser --test usx_text     # ~15 s
run -p usfm_parser --test verse_ends   # ~18 s
run -p usfm_parser --test spans        # ~57 s

# Malformed input, where the lexer's cursor ends up in the least ordinary
# places. insta reads its snapshots from disk, which Miri's isolation
# forbids.                                                            ~73 s
(
  export MIRIFLAGS="${MIRIFLAGS:-} -Zmiri-disable-isolation"
  run -p usfm_parser --test recovery -- --exact \
    stray_backslash \
    unmatched_milestone_end \
    nested_marker_not_nested \
    unknown_custom_milestone \
    unknown_custom_marker \
    empty_word \
    escaped_quote_in_attribute_value \
    unterminated_attribute_value \
    newline_in_attributes \
    malformed_verse_number \
    character_style_not_closed_at_eof
)

echo "miri: clean"
