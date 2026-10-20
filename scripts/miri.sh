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
# Miri interprets each program on one host thread, so the test harness's
# threads buy nothing inside a suite. The suites are separate processes,
# though, so this script builds them all first and then runs them side by
# side, one per CPU (`MIRI_JOBS` overrides), longest first, and prints each
# suite's output whole once all have finished. Run one after another they took
# about 5 min on the 4-vCPU agent VM and 3 min 20 s in CI (2026-10-20); side
# by side the wall time is about the longest suite's. Most of every suite's
# fixed cost is `DEFAULT_STYLESHEET`: building its 335 rules and their ~6 000
# strings takes about 11 s under Miri, once per suite, and again each time a
# test clones the sheet or the parser derives a style (`Arc::make_mut`). A
# suite added here still has to earn it.
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

# Suites, longest first so the long ones are not the last to start. `"$1"` is
# `build` or `launch` below; `--no-isolation` lets a suite read files (insta
# reads its snapshots from disk, which Miri's isolation forbids).
suites() {
  # Whole-document parses: the lexer over real markup, and the span
  # invariants checked mechanically.
  "$1" -p usfm_parser --test usx_text     # ~46 s
  "$1" -p usfm_parser --test spans        # ~59 s
  "$1" -p usfm_parser --test whitespace   # ~17 s
  "$1" -p usfm_parser --test verse_ends   # ~18 s
  "$1" -p usfm_parser --test attributes   # ~13 s

  # Malformed input, where the lexer's cursor ends up in the least ordinary
  # places. insta reads its snapshots from disk, which Miri's isolation
  # forbids.                                                            ~57 s
  "$1" --no-isolation -p usfm_parser --test recovery -- --exact \
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
  "$1" --no-isolation -p usfm_usx --test reader -- --exact \
    usx_not_well_formed \
    a_byte_order_mark_and_a_declaration_are_read_past \
    dbl_pretty_printing_is_formatting \
    a_notes_indentation_is_not_a_space \
    text_is_read_by_the_ast_whitespace_rules \
    attributes_are_read_in_the_canonical_form \
    every_attribute_the_writer_writes_is_read \
    what_the_writer_writes_reads_back

  # `write_text` / `write_attribute_value`: two byte scans over a `&str` that
  # slice at the indices they stop on — `\\`, `|`, the two bytes of U+00A0, and
  # `"` inside a quoted attribute value (ticket 25).                     ~15 s
  "$1" -p usfm_codegen --lib -- --exact \
    usfm::tests::text_is_escaped_the_way_the_parser_reads_it_back \
    usfm::tests::a_quote_in_an_attribute_value_is_escaped \
    usfm::tests::a_stray_backslash_comes_back_escaped \
    usfm::tests::the_default_attribute_is_written_bare

  # The lexer's own unit tests, including `lexer::source`, plus the parser and
  # style unit tests. The `--skip text_replacements` this line carried until
  # ticket 15 is gone with the module.                                  ~20 s
  "$1" -p usfm_parser --lib

  # `string_parser` (through `number`), `cursor`, the visitors, `text`. The
  # `reference` tests were here until ticket 22 moved `ReferenceIndex` to
  # `usfm_semantic`; they are not re-added below, for the reason that crate is
  # skipped — the index walks node references and clones a `NumberList`, and
  # slices no bytes.                                                      ~9 s
  "$1" -p usfm_ast --lib

  # `Diagnostic::render` / `to_json_line` over a `LineIndex`, and the JSON
  # escaping that walks a `&str` char by char.                            ~5 s
  "$1" -p usfm_diagnostics --lib

  # `write_escaped`: a byte-table scan over a `&str` that slices at the indices
  # it stops on, plus the reader round trip (ticket 13).                  ~3 s
  "$1" -p usfm_usx --lib

  # `Span` and `LineIndex`: byte offsets sliced out of a `&str`.          ~1 s
  "$1" -p usfm_span --lib

  # `write_escaped` / `write_escaped_attribute`: the same byte-table scan over a
  # `&str`, slicing at the indices it stops on (ticket 14).               ~1 s
  # Only the `escape` tests: the rest of `usfm_html`'s lib suite parses whole
  # documents through the HTML writer, which is 14 s of Miri for byte handling
  # the parser suites below already cover (whole suite: 15 s).
  "$1" -p usfm_html --lib escape::
}

# Build every suite first, one after another, so the runs below start from a
# finished build instead of contending for cargo's build lock.
build() {
  [[ $1 == --no-isolation ]] && shift
  cargo +nightly miri test --no-run --quiet "$@"
}

parallel="${MIRI_JOBS:-$(nproc)}"
logs="$(mktemp -d)"
trap 'rm -rf "$logs"' EXIT
count=0

# Each suite in the background, its output to a file of its own, at most
# `$parallel` at once.
launch() {
  local flags="${MIRIFLAGS:-}"
  if [[ $1 == --no-isolation ]]; then
    flags="$flags -Zmiri-disable-isolation"
    shift
  fi
  while (( $(jobs -rp | wc -l) >= parallel )); do
    wait -n || true
  done
  count=$((count + 1))
  local log="$logs/$(printf '%02d' "$count")"
  (
    echo "==> cargo +nightly miri test $*"
    start=$SECONDS
    status=0
    MIRIFLAGS="$flags" cargo +nightly miri test "$@" || status=$?
    echo "<== $((SECONDS - start)) s, exit $status"
    echo "$status" >"$log.status"
  ) >"$log.out" 2>&1 &
}

echo "miri: building the suites"
suites build
suites launch
echo "miri: running $count suites, $parallel at a time"
wait

failed=0
for status in "$logs"/*.status; do
  cat "${status%.status}.out"
  [[ $(cat "$status") == 0 ]] || failed=1
done
for out in "$logs"/*.out; do
  [[ -f ${out%.out}.status ]] || { cat "$out"; failed=1; }
done
if (( failed )); then
  echo "miri: a suite failed (see above)" >&2
  exit 1
fi
echo "miri: clean"
