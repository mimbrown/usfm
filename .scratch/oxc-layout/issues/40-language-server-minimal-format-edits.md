# 40. Language server: minimal-diff formatting edits

Status: needs-triage
Milestone: after M6

`textDocument/formatting` returns one `TextEdit` replacing the whole
document (ticket 31). Correct, and what `formatOnSave` needs, but a client
that applies it as one replacement loses cursor position, folding and the
undo granularity of a line-wise edit, and some show a flicker. Ticket 31
called a diff a follow-up "if the client flickers"; nobody has reported
one yet.

- A line-based diff (the `similar` or `dissimilar` crate, or a hand-rolled
  LCS over lines — the writer's output is line-shaped) between the stored
  text and `usfm_codegen`'s output, emitted as one `TextEdit` per changed
  hunk, ranges through `convert::range`.
- Tests: the edits applied in order reproduce the writer's output byte for
  byte on the benchmark corpus (which `--check` already proves is a fixed
  point, so most books produce zero edits) and on a document with one
  changed line.

Done when the stdio test asserts a one-line change produces a one-hunk edit
and the gate is green.
