# 06. Fuzz target

Status: ready-for-agent
Milestone: M2
Blocked by: 01

`tasks/fuzz` with cargo-fuzz: parse arbitrary bytes (lossy UTF-8 and valid UTF-8
targets), assert no panic, the span invariants from `tests/spans.rs`, and that
`to_usx_string` output is well-formed XML. Seed with the tcdocs inputs. Every
finding becomes a `recovery.rs` test before it is fixed.

Done when both targets have run 10 minutes clean.
