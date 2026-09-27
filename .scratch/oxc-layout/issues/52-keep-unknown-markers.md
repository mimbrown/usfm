# 52. Keep an unknown marker instead of dropping it?

Status: ready-for-human
Milestone: after M7 (render gaps)

Found by mapping Shahkar-Urdu-Apps/render (gap 1). With no stylesheet entry
for it, the parser drops a marker and keeps its text:

- `\zgrk logos\zgrk*` reads as plain `logos` (`unknown-custom-marker`,
  Warning), so the style and everything an output keys on it is gone;
- `\app-mt1 Title` (render's per-medium variant of `\mt1`, kept for digital
  output and dropped for print, `render:lib/modifyUSFM.ts:132`) reads as
  `<para style="p">Title</para>`: the marker is gone and the text is in an
  implicit `\p`, so the distinction the project relies on is lost.

Paratext keeps both: an unknown marker is a node with that style name (a
character style if it has a closer in sight, otherwise a paragraph), flagged
as unknown. render's own parser does the same (`getMarker` falls back to a
Character style). Ticket 51 fixes the case where the project's `custom.sty`
is at hand; this ticket is about what happens when it is not.

**The question.** Should the parser derive a style for an unknown marker, as
it already does for unknown *milestones* (`\zaln-s`, kept since Phase 2),
rather than drop it? What it would change:

- the tree and every output for inputs that have unknown markers today (the
  diagnostic would stay, perhaps as a different code or severity);
- tcdocs: which reference files write an unknown marker, and whether they
  keep it, has to be checked case by case before deciding;
- round trip: a derived style is written back as the marker, which the
  current drop cannot do.

Options: (a) keep every unknown marker as a derived style (Paratext's rule);
(b) keep only `\z…` markers, which USFM reserves for extensions, and drop
the rest; (c) leave it as is and rely on ticket 51 / 53 to load the sheet.
Recommendation: (b) first, since `\z` is the spec's own extension space.
