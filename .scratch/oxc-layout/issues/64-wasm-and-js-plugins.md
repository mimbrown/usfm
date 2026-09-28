# 64. The parser in WASM, driven from JS

Status: needs-triage
Milestone: after M7 (publishing foundation)

Michael, 2026-09-28, long term: compile the parser to WASM and drive the
whole process from JS. oxc does this by handing its tree to JS through a
shared binary buffer ("raw transfer") and running JS lint plugins over it.
We would need:
- a binary layout for our tree;
- a JS reader for it;
- a plugin API.

Not planned for now: render goes Rust end to end first (ticket 57). Kept so
the idea is not lost.
