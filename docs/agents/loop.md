# The loop

How an unattended session works through `.scratch/<effort>/`. The orchestrator is
the session itself; it delegates each ticket to one Opus subagent
(`Agent` with `model: "opus"`) and reviews what comes back. One ticket at a time:
the tickets are a chain, and parallel edits to a crate split collide.

## One iteration

1. **Frontier.** In `.scratch/<effort>/issues/`, take the lowest-numbered ticket
   that is `Status: ready-for-agent` and whose `Blocked by:` tickets are all
   `resolved`. Skip `needs-info` and `ready-for-human`.
2. **Claim.** Set `Status: claimed`, commit, push.
3. **Delegate.** Give the subagent the ticket path, the spec, the ADR and this
   file. It implements, runs `scripts/gate.sh`, and reports what it changed, what
   it did not do, and anything that surprised it. It does not commit.
4. **Review.** The orchestrator reads the diff, not the report. It reruns
   `scripts/gate.sh` itself. A parser gap is fixed, never patched around: no new
   tcdocs patch, baseline entry, `#[ignore]`, `#[allow]` without a reason, or
   deleted test to get green. If the work is wrong, send it back with specifics
   (at most three rounds, then see Stop).
5. **Land.** Commit, push to `main`, wait for CI (`gh run watch`). Red CI is
   fixed or reverted before anything else happens.
6. **Resolve.** Append `## Answer` (what landed, commit hash, follow-ups), set
   `Status: resolved`. A follow-up is a new ticket, not scope added to this one.
   Update CLAUDE.md and `docs/plans/hardening.md` when a status line in them
   changed.
7. **Milestone boundary.** When a milestone's last ticket resolves, check its exit
   criteria in the spec one by one, record the result in the spec, then write the
   next milestone's tickets from what is now known. Commit them before starting.

## Stop

Stop and leave a note under `## Comments` on the ticket (and in the final
message) when:

- a ticket fails review three times, or the gate cannot be made green without one
  of the shortcuts above;
- the work would contradict the ADR or a decision in `hardening.md` (D1–D7);
- a ticket needs something only Michael has (a licence call, a real project
  file, a credential);
- the frontier is empty.

A blocked ticket does not stop the loop if another ticket is unblocked: mark it
`ready-for-human` with the question, and move on.

## Invariants

The spec's invariant holds after every commit: suites green, tcdocs at its
recorded numbers, baseline empty. The benchmark numbers in `docs/benchmarks.md`
(once M2 lands) are rerun at each milestone boundary; a regression over the spec's
threshold is a ticket before the next milestone starts.
