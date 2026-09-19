# usfm-grammar fixtures

Two test directories vendored from
[Bridgeconn/usfm-grammar](https://github.com/Bridgeconn/usfm-grammar), commit
`4ee1b91c9b725f7f5be3654805ab6447b111da58`, fetched 2026-09-19. MIT, © 2021
Bridge Connectivity Solutions; `LICENSE` here is that repository's licence
file, copied verbatim. Scripture excerpts inside the inputs belong to their
respective publishers. See `NOTICE.md` at the repository root.

Only the two directories that cover ground `tcdocs/` does not are vendored, and
only the files we use: usfm-grammar's own `origin.json` outputs are its parser's
format, not USX, so they are not an oracle for us and are left upstream.

## `bugfixes/`

Sixteen regression cases from usfm-grammar's issue tracker, in tcdocs' own
layout (`metadata.xml` with a `<validated>` verdict, `origin.usfm`,
`origin.xml`). They are a second root of the `usfm_tests` harness, under the
category `usfm-grammar/bugfixes`:

```bash
cargo run -p usfm_tests usfm-grammar/bugfixes
cargo run -p usfm_tests -- --show usfm-grammar/bugfixes/q4
```

Three cases (`multiple-sr`, `nestedchar-footnote`, `rem_with_char`) ship without
an `origin.xml`. The harness runs them as "must parse with no error
diagnostics", which is what their `<validated>pass</validated>` asserts; see
`tests/src/lib.rs`.

Five of them are read through a patch in `tests/tcdocs-patches/usfm-grammar/`,
each with its rationale above the diff: usfm-grammar ends a verse in a table at
the end of the cell whose text it ran out in, wraps `\list-s`/`\list-e` in a
`<list>` element `usx.rnc` does not have, and reads `\vid` as paragraph
attributes rather than the milestone `usx.rnc` declares it to be.

## `autofix/`

Twelve deliberately malformed inputs (plus one large real-world file,
`fr-textTranslation-FR_TLX.txt`, which despite its extension is USFM). They
carry no expected output at all — upstream uses them to exercise its
auto-correction pass. Here they are recovery shapes, covered by tests in
`usfm_parser/tests/recovery.rs` named after the file: seven assert the
`Diagnostic` code we report, four snapshot the tree for a shape that is
ordinary USFM and reports nothing, and `wrong_book_code.usfm` is byte-for-byte
the input of the `missing_book_code` test that was already there.

Every `autofix` input and every `bugfixes/*/origin.usfm` is also a fuzz seed;
`tasks/fuzz/seed.sh` copies them into both corpora as
`usfm-grammar__<dir>__<name>.usfm`.
