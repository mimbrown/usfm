# 33. Delete `wip/` and decide the lexicon feature

Status: ready-for-human
Milestone: M6
Blocked by: 32

With the server rebuilt, `wip/usfm_language_server` has nothing left to
give and can be deleted (`git rm`), with CLAUDE.md, NOTICE.md and the root
`Cargo.toml` `exclude` updated.

`wip/data_layer` is the SQLite lexicon (`rusqlite`, a `schema.rs`) the old
server used for a word-lookup feature. Nothing in the rebuilt server uses
it. Question for Michael: is the lexicon feature wanted back (then it is an
M6 follow-up ticket with a design: what the lexicon holds, where it comes
from, and whether it belongs in the server or in a separate tool), or can
`wip/data_layer` be deleted with the old server? Until answered, both stay
parked and this ticket blocks nothing.

## Comments

2026-09-20, orchestrator: the loop stopped here — tickets 30–32 are
resolved, this ticket and ticket 10 are the only open ones and both are
`ready-for-human`, so the frontier is empty. M6's exit criteria are recorded
in the spec: every one is met except the deletion of `wip/usfm_language_server`,
which this ticket couples with the `wip/data_layer` question and reserves for
Michael. The deletion itself has no open question left — nothing in
`apps/usfm_language_server` uses the parked code, and the VS Code extension
now spawns the new binary — so on a "delete both" answer this ticket is a
one-line `git rm -r wip/` plus the CLAUDE.md, NOTICE.md and root `Cargo.toml`
`exclude` edits; on a "lexicon wanted" answer, `wip/usfm_language_server`
still goes and `wip/data_layer` stays for the follow-up ticket that designs
the feature.
