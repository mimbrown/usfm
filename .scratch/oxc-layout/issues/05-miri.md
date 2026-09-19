# 05. Miri on the unsafe code

Status: ready-for-agent
Milestone: M2
Blocked by: 01

41 `unsafe` blocks in `lexer/source.rs`, 3 in `lexer/mod.rs`, 1 in `cursor.rs`,
plus `usfm_ast/src/string_parser.rs`. Run `cargo +nightly miri test` over the
lexer, whitespace and recovery suites (trim any test too slow for Miri). Every
block gets a `// SAFETY:` comment or is replaced with safe code; after 04, replace
any block whose removal costs under 2%.

Done when Miri is clean and is a CI job.
