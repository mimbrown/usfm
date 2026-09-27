# 51. `--custom-stylesheet`: a project's `custom.sty` over the default, in the CLI

Status: resolved
Milestone: after M7 (render gaps)

Found by mapping Shahkar-Urdu-Apps/render (2026-09-27,
`reports/render-gap-map.md` in the project files, gap 1). A Paratext project
defines its own markers in `custom.sty` — render's content uses `\zgrk`,
`\zheb`, `\zeng`, `\zarab` and `\zgrk2` as font switches — and without them
the parser drops each one as `unknown-custom-marker`. The language server
already reads a `custom.sty` *over* the default sheet (ticket 30,
`StyleSheet::extend_from_str`, Paratext's amendment semantics), but the CLI's
`--stylesheet` *replaces* the default, so a pipeline could not say "usfm.sty
plus this project's custom.sty".

- `usfm parse` and `usfm format` take `--custom-stylesheet FILE`, read with
  `extend_from_str` over the `--stylesheet` sheet or the built-in one.
- `parse` watches the file in `--watch`; `format` fails when it cannot read
  it, as it does for `--stylesheet`.
- Not auto-detected beside the input the way the language server does: that
  is a behaviour change for existing command lines, and ticket 53's project
  reader is the place a project's sheet is found.

## Answer

Landed with the ticket (the PR that files 51–57). `driver::extended_sheet`
is the one composition both commands use; `apps/usfm_cli/tests/cli.rs` has
`custom_stylesheet_extends_the_built_in_one`,
`format_keeps_a_custom_marker_with_custom_stylesheet` and
`format_refuses_a_missing_custom_stylesheet`.
