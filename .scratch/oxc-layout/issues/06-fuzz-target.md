# 06. Fuzz target

Status: resolved
Milestone: M2
Blocked by: 01

`tasks/fuzz` with cargo-fuzz: parse arbitrary bytes (lossy UTF-8 and valid UTF-8
targets), assert no panic, the span invariants from `tests/spans.rs`, and that
`to_usx_string` output is well-formed XML. Seed with the tcdocs inputs. Every
finding becomes a `recovery.rs` test before it is fixed.

Done when both targets have run 10 minutes clean.

## Answer

Landed in `bfaa57f` (PR #9, 2026-09-19). `tasks/fuzz` (`usfm_fuzz`, outside the
workspace: nightly + sanitizer) with `parse_lossy` and `parse_utf8`, each
asserting no panic, the span invariants (now `usfm_parser::span_check`, shared
with `tests/spans.rs` behind the `testing` feature) and that `to_usx_string`
output parses back as XML. Seeded from the 259 tcdocs inputs (`seed.sh`,
files over 64 KiB truncated to whole lines). Both ran 10 minutes clean on the
final code (49 and 83 exec/s, no crashes).

Seven findings before that, all fixed with tests: two checker rules for
implicit `\p`/`\tc1` nodes; a `\periph` title span inverted by a `\v` on the
title line (parser fix); C0 controls and U+FFFE/U+FFFF reaching USX (writer
now replaces them with U+FFFD, byte-table scan so `parse_usx` stays at
parity); two new codes, `duplicate-attribute` and `malformed-attribute-name`
(Error), with the USX writers dropping what XML cannot carry.

Follow-ups, not done here:
- The HTML serializer has the same control-character hazard; the fuzz targets
  check USX only. Ticket it with an HTML well-formedness check.
- `docs/benchmarks.md` was not edited; the writer change measured at parity
  on whole-corpus and up to 4% under on three classes on a machine that had
  just been fuzzing. Rerun at the M2 boundary.
