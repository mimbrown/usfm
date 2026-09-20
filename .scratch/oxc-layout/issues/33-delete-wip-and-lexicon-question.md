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
