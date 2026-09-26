# 33. Delete `wip/` and decide the lexicon feature

Status: resolved
Milestone: M6
Blocked by: 10

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

2026-09-26, Michael (via the orchestrator): **delete both.** The lexicon
feature is not coming back for now, so `wip/usfm_language_server` and
`wip/data_layer` both go (`git rm -r wip/`), with CLAUDE.md, NOTICE.md and
the root `Cargo.toml` `exclude` updated, and the hardening plan's Phase 5
lexicon line marked as decided against. Unblocked; runs after ticket 10 so
the two PRs do not race.

## Answer

Landed via PR #44 (2026-09-26). `git rm -r wip/`: the old language server
and `data_layer` (9 files, 1390 lines) are gone, and the root `Cargo.toml`
`exclude` is `["tasks/fuzz"]` alone; `Cargo.lock` is unchanged, since the
crates were never in the workspace. Every mention outside `wip/` was
rewritten to describe the present (CLAUDE.md's stream entry and structure
tree, the server's manifest and module doc, `extension.ts`), or kept as
dated history where the history is the point (ticket 01's checklist entry
in `hardening.md`, the ADR's layout note, the spec's M6 record — which now
says the deletion criterion is met and M6 is closed). The lexicon feature
is decided against, recorded on `hardening.md`'s Phase 5 line and open
question 5; reopening it takes a new ticket with a design. `NOTICE.md`
needed nothing. Left in `vscode/`: the `usfm.dict` custom editor, which is
client-only and still works, and a dead `lexicon` key in the
`usfm.config.json` schema that only the deleted server read — removing it
would flag existing configs (`additionalProperties: false`), so it stays
until the schema is next revised. Gate exit 0.
