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
# done: **about 5 min** (300 s wall on 2026-09-27; per-suite times in the
# comments below), with the reader's 31 s in it. That is the whole of the CI
# step's 5-minute budget, and not only because of the reader: `usx_text`
# takes 46 s where this script said ~15 s, and as long at the M6 close commit
# too, so the VM or the nightly toolchain moved rather than the code. Without
# the reader the total is about 4 min 30 s today. A suite added
# here has to earn it; the next one should come with a cut elsewhere.
#
# Not run in full, on purpose:
# * `usfm_usx --test reader` (ticket 49) — eight of its 25 tests; the block
#   below says which and why.
# * `usfm_codegen` (ticket 25) — four of its 19 lib tests, the ones that drive
#   the two byte scans (`write_text`, `write_attribute_value`). Those two are
#   the crate's only byte handling: everything else it writes is a marker name
#   out of the stylesheet or a `Display` impl, and every test parses a whole
#   document first, which is 48 s of Miri for the parser coverage the suites
#   below already give (whole lib suite: 49 s, against 15 s for these four).
#
# Not run, on purpose:
# * `usfm_parser --test snapshot` — the insta corpus, hundreds of files of I/O
#   and no byte handling the suites below do not already reach.
# * `usfm_tests` (tcdocs) — 260 cases, hours under Miri.
# * `usfm_json` (ticket 16) — its own code builds a `serde_json::Value` node by
#   node and slices no bytes; all the time would go into `serde_json`'s number
#   and string writers, which are not this repo's code and are slow under Miri.
#   The `usfm_ast` suite above already covers every `&str` the JSON writer
#   hands on.
# * `usfm_semantic` (ticket 19) — it slices no bytes at all. A check there reads
#   a built tree: it compares a `BookCode`, copies a `Span` a node already
#   carries and formats a message. There is no `unsafe`, no indexing into a
#   `&str` and no source text in the crate, so Miri would only re-run a handful
#   of assertions over hand-built nodes. The same goes for `ReferenceIndex`,
#   which arrived with ticket 22: it walks `NodeRef`s, pushes child indices and
#   clones a `NumberList`. Revisit if a check ever reads source text.
# * `usfm_language_server` (ticket 30) — Miri cannot run it. Its tests are a
#   tokio runtime and, for the integration test, a spawned process; Miri has no
#   real threads to run the reactor on and forbids `Command::spawn`. There is
#   nothing here to check either: the crate slices no bytes. The one thing it
#   computes from source text is a UTF-16 column, which is
#   `usfm_span::LineIndex::line_col_utf16` and runs under the `usfm_span` suite
#   at the top of this script.
# * `usfm_pipeline` — its lib suite is the text replacements, which spend all
#   their time inside the `regex` crate; that is not this repo's code, and it
#   cost 66 s of the parser's 86 s while it lived there (ticket 15 moved it).
# * 93 of the 103 `recovery` tests — the 10 kept are the ones whose input drives
#   the lexer somewhere unusual (a lone `\`, an unterminated quote, an escaped
#   one, a marker name with `-` or `_`, a newline inside an attribute list, EOF
#   inside a character style, a malformed number through `string_parser`).
#   `empty_word` was an eleventh until ticket 21 moved it to `usfm_semantic`;
#   its `\w |lemma="x"\w*` is attribute lexing, which the whole `attributes`
#   suite above covers. The
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

# `string_parser` (through `number`), `cursor`, the visitors, `text`. The
# `reference` tests were here until ticket 22 moved `ReferenceIndex` to
# `usfm_semantic`; they are not re-added below, for the reason that crate is
# skipped — the index walks node references and clones a `NumberList`, and
# slices no bytes.                                                      ~9 s
run -p usfm_ast --lib

# `write_escaped`: a byte-table scan over a `&str` that slices at the indices
# it stops on, plus the reader round trip (ticket 13).                  ~3 s
run -p usfm_usx --lib

# The USX reader (ticket 45, Miri since ticket 49): `roxmltree` has no
# `unsafe` (`#![forbid(unsafe_code)]`), but the reader slices the source with
# the byte ranges it hands back — every span, every borrowed attribute value
# and text run — and collapses whitespace over `&str`s. insta reads its
# snapshots from disk, which Miri's isolation forbids.                  ~31 s
# Eight of the suite's 25 tests: the ones whose input reaches the reader's
# byte handling somewhere unusual — XML that is not well formed, a byte-order
# mark and a declaration read past, DBL's pretty-printing inside mixed
# content, a note's indentation, the whitespace rules, every attribute shape
# the writer writes and their spans, and the writer's own output read back.
# The other 16 are one USX vocabulary rule each over a few lines of ordinary
# XML, or a machine.py Tes book, and add 110 s between them (the suite minus
# 1JN is 140 s; about 11 s of any run is the default stylesheet built once
# under Miri), which the CI step's budget does not have. Skipped even from a
# full run: the machine.py WEB test, which reads 1JN (115 KB) and had not
# finished after 15 minutes.
(
  export MIRIFLAGS="${MIRIFLAGS:-} -Zmiri-disable-isolation"
  run -p usfm_usx --test reader -- --exact \
    usx_not_well_formed \
    a_byte_order_mark_and_a_declaration_are_read_past \
    dbl_pretty_printing_is_formatting \
    a_notes_indentation_is_not_a_space \
    text_is_read_by_the_ast_whitespace_rules \
    attributes_are_read_in_the_canonical_form \
    every_attribute_the_writer_writes_is_read \
    what_the_writer_writes_reads_back
)

# `write_escaped` / `write_escaped_attribute`: the same byte-table scan over a
# `&str`, slicing at the indices it stops on (ticket 14).               ~1 s
# Only the `escape` tests: the rest of `usfm_html`'s lib suite parses whole
# documents through the HTML writer, which is 14 s of Miri for byte handling
# the parser suites below already cover (whole suite: 15 s).
run -p usfm_html --lib escape::

# `write_text` / `write_attribute_value`: two byte scans over a `&str` that
# slice at the indices they stop on — `\\`, `|`, the two bytes of U+00A0, and
# `"` inside a quoted attribute value (ticket 25).                     ~15 s
run -p usfm_codegen --lib -- --exact \
  usfm::tests::text_is_escaped_the_way_the_parser_reads_it_back \
  usfm::tests::a_quote_in_an_attribute_value_is_escaped \
  usfm::tests::a_stray_backslash_comes_back_escaped \
  usfm::tests::the_default_attribute_is_written_bare

# The lexer's own unit tests, including `lexer::source`, plus the parser and
# style unit tests. The `--skip text_replacements` this line carried until
# ticket 15 is gone with the module.                                  ~20 s
run -p usfm_parser --lib

# Whole-document parses: the lexer over real markup, and the span
# invariants checked mechanically.
run -p usfm_parser --test whitespace   # ~17 s
run -p usfm_parser --test attributes   # ~13 s
run -p usfm_parser --test usx_text     # ~46 s
run -p usfm_parser --test verse_ends   # ~18 s
run -p usfm_parser --test spans        # ~59 s

# Malformed input, where the lexer's cursor ends up in the least ordinary
# places. insta reads its snapshots from disk, which Miri's isolation
# forbids.                                                            ~57 s
(
  export MIRIFLAGS="${MIRIFLAGS:-} -Zmiri-disable-isolation"
  run -p usfm_parser --test recovery -- --exact \
    stray_backslash \
    unmatched_milestone_end \
    nested_marker_not_nested \
    unknown_custom_milestone \
    unknown_custom_marker \
    escaped_quote_in_attribute_value \
    unterminated_attribute_value \
    newline_in_attributes \
    malformed_verse_number \
    character_style_not_closed_at_eof
)

echo "miri: clean"
