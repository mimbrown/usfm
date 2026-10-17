# Third-party material

The code in this crate is MIT licensed (see `LICENSE`). `usfm.sty` is not:
it is Paratext's default stylesheet for combined study Bible projects
(`usfm_sb.sty`, version 3.0.11), by United Bible Societies and SIL
International, byte for byte the copy the USFM/USX technical committee's
test suite carries at `grammar/usfm_sb.sty`
(https://github.com/usfm-bible/tcdocs). Copyright © 2022-2023 the members
of the USFM/USX technical committee; licensed under the Creative Commons
Attribution 4.0 International licence (CC BY 4.0,
https://creativecommons.org/licenses/by/4.0/).

The file is unchanged. `build.rs` compiles it into `DEFAULT_STYLESHEET`
together with `usfm-extra.sty`, this crate's own additions and corrections
(MIT), which are read over it. Documentation:
https://ubsicap.github.io/usfm/
