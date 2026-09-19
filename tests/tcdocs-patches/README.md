# Reference patches

One unified diff per conformance test, at `<test name>.patch`, applied to that
test's `origin.xml` before the harness compares it with the parser's output.
`tests/src/lib.rs` (`TestCase::read_expected_usx`) applies them. The directory
is named after tcdocs because tcdocs was the only root when it was created; it
covers every root the harness discovers, so a usfm-grammar case's patch is at
`usfm-grammar/bugfixes/<case>.patch`.

A patch exists only for a difference that is settled, and says which kind it
is in the text above the `---` line (the harness ignores that preamble):

- **Reference quirk**: the file is wrong or inconsistent. A hand-written file
  with a raw newline where every other line break became a space; two files
  in the same test that disagree; a space Paratext invents; a shape that
  contradicts the stylesheet and what Paratext writes elsewhere in the suite.
- **Accepted deviation**: the parser differs from the reference on purpose,
  and the rationale says why and points at the rule it follows instead.

A parser gap is fixed in the parser, never patched. Cite evidence in the
rationale: another tcdocs case, the stylesheet entry, `tcdocs/grammar/usx.rnc`,
the whitespace rules on `Text`, the plan.

The harness keeps the directory honest:

- A patch that no longer applies fails the test (tcdocs changed: re-read the
  case, then fix or delete the patch).
- A patch the parser no longer needs fails the test too: when the output
  matches the reference *as checked in*, the patch is redundant and must be
  deleted, like a stale baseline entry.

Patches are written against the file with its byte-order mark removed and
`\r\n` turned into `\n`; the harness normalises the same way. To write one:

```bash
cargo run -p usfm_tests -- --show specExamples/footnote   # diagnostics, output, expected
sed 's/\r$//' tcdocs/tests/specExamples/footnote/origin.xml | sed '1s/^\xEF\xBB\xBF//' > /tmp/a.xml
cp /tmp/a.xml /tmp/b.xml && $EDITOR /tmp/b.xml
diff -u /tmp/a.xml /tmp/b.xml | sed '1s|.*|--- origin.xml|;2s|.*|+++ origin.xml|' >> tests/tcdocs-patches/specExamples/footnote.patch
```

with the rationale written above the diff.
