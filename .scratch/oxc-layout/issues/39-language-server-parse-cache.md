# 39. Language server: one parse per document version, not per request

Status: needs-triage
Milestone: after M6

`apps/usfm_language_server` re-parses the stored text on every request —
diagnostics on change, then again for a hover, a completion, a code action
or a format on the same version. A book parses in milliseconds
(`docs/benchmarks.md`), so nothing has measured a problem; this is a
follow-up only if one is measured (a large book with hover on every mouse
move, or a client that sends completion on every keystroke).

- Measure first: `RUST_LOG`-style timing per request, or the bench corpus's
  largest book through the stdio test with a hover per line.
- If wanted: cache `(version, ParseResult)` per `Uri` in `Documents`, keyed
  by the `textDocument.version` the client sends; `ParseResult` borrows the
  source, so either `into_owned()` it or keep the `String` beside it in an
  `Arc`. Invalidate on `did_change`.

Done when the measurement is in `docs/benchmarks.md` and, if the cache went
in, the stdio test still passes and the server holds one parse per version.
