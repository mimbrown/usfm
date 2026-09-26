# 43. Recovering usfm-js's unclosed milestones (the "old format")

Status: resolved
Milestone: after M6

Found by ticket 10. unfoldingWord's aligned texts from before USFM 3.0 was
final — usfm-js's `*.oldformat.usfm` test resources, and the whole book
`large.usfm` that ticket 10 vendored (after closing its milestones) — write a
start milestone's attribute list to the end of the line and never close it:

```
\id ACT
\c 1
\p
\v 1
\zaln-s |x-strong="G35880" x-content="τὸν"
\w The|x-occurrence="1"\w*
\zaln-e\*
\k-s |x-tw="rc://x"
\w b|lemma="b"\w*
\k-e\*
```

This is not USFM 3 (a milestone ends at `\*`), and tcdocs agrees: its
`usfmjsTests/*.oldformat` cases with this shape are `validated=fail`. So the
Errors are right and this is **not a bug**. What is poor is the recovery:

```
warning[unknown-custom-marker]: custom marker `\zaln-s` is not in the stylesheet; dropped
error[unexpected-pipe]: `|` outside a character style; kept as text
info[unknown-custom-milestone]: custom milestone `\zaln-e` is not in the stylesheet
error[unknown-marker]: unknown marker `\k-s`
error[unexpected-pipe]: `|` outside a character style; kept as text
```

- the start milestone is dropped while its end is kept, so the tree has an
  `\zaln-e` / `\k-e` with no start;
- the attribute list becomes verse text (`|x-strong="G35880" x-content="τὸν"`),
  and a `//` inside a value becomes an `OptBreak`;
- the same shape gets two different codes and severities depending on the
  marker (`unknown-custom-marker` Warning for `\zaln-s`, `unknown-marker`
  Error for `\k-s`, whose `\k` base is a known style).

Over `large.usfm` as upstream wrote it that is 19 140 `unexpected-pipe`
Errors and every milestone's attributes in the text.

A better recovery, if wanted: a start milestone followed by `|` whose
attribute list reaches the end of the line with no `\*` is read as closed at
the line break, keeps its attributes, and reports one new Error
(`milestone-not-closed`, say) on the marker. It would need a `recovery.rs`
test per shape, would change no tcdocs verdict (those cases would still
report an Error), and would change what `usfm_codegen` writes for them (the
closed spelling), which the round trip allows.

Question for triage: is recovering a pre-USFM-3 dialect worth a rule, given
that usfm-js itself converts it and `tasks/benchmark/corpus/tools/usfmjs_oldformat.py`
does the same edit mechanically? Related: the hardening plan's unchecked
"a malformed real-world corpus" box — the upstream `large.usfm` is one.

## Answer

Picked by Michael on 2026-09-26: yes, worth a rule.

A start milestone (`-s`) followed by `|` whose attribute list reaches a line
break or the end of input, with no `\*`, is closed there
(`take_unclosed_milestone` in `parser.rs`). It keeps its attributes, the line
break stays where it is (so the tree is the one the closed spelling gives),
and it reports the existing `milestone-not-closed` Error on the marker —
rather than a new code, since that is what an unclosed milestone already
reports. Both paths read it: an unknown marker (`take_unknown_milestone`,
the first `\zaln-s`, and every derived `\k-s`) and a known one
(`parse_milestone_node`, every `\zaln-s` after the first registers it, and
`\qt-s`). Anything else is unchanged: a list that stops at a marker on its
own line is the old `milestone-not-closed`, and a marker without `-s` stays
unknown.

Measured on the aligned corpus with its `\*` taken off again (the inverse of
`usfmjs_oldformat.py`): the ULT reads to exactly the committed file's tree
with 19 140 `milestone-not-closed` and nothing else new, the UGNT with 156.
That is now a test (`the_old_format_aligned_books_read_as_their_closed_spelling`),
beside a snapshot of each shape (`recovery__milestone_not_closed__at_line_end`),
a tree-equality test on a small sample, and a negative case. tcdocs is
unchanged: its `*.oldformat` cases still report an Error.

The hardening plan's "malformed real-world corpus" box is not ticked by
this: upstream's `large.usfm` would be one, but it is not committed (only its
closed form is), and the test above rebuilds it from the committed file.
