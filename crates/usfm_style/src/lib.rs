//! The USFM stylesheet: [`StyleSheet`], its [`StyleRule`]s, and the
//! [`DEFAULT_STYLESHEET`] every reader resolves a marker against when it is
//! given no other.
//!
//! The default sheet lives here rather than in the parser because it is not
//! the parser's: `usfm_usx`'s reader resolves a USX `style` through it too
//! (ticket 45), and an output crate does not depend on the parser. It is
//! generated from `usfm.sty` and `usfm-extra.sty` by `build.rs` into
//! `OUT_DIR`, so nothing generated is checked in, and `usfm_parser` re-exports
//! it under the name it has always had there.

mod sheet;

pub use sheet::*;

/// The generated default stylesheet.
mod generated {
    include!(concat!(env!("OUT_DIR"), "/default_stylesheet.rs"));
}

pub use generated::DEFAULT_STYLESHEET;

#[cfg(test)]
mod tests {
    use super::*;

    /// What `usfm.sty` declares agrees with the spec's defaults, `\fig` has
    /// none, and a milestone's end form has its start's.
    #[test]
    fn the_default_sheet_declares_its_default_attributes() {
        let sheet = &DEFAULT_STYLESHEET;
        for (marker, name) in [
            ("w", Some("lemma")),
            ("rb", Some("gloss")),
            ("xt", Some("link-href")),
            ("jmp", Some("link-href")),
            ("qt-s", Some("who")),
            ("qt2-e", Some("who")),
            ("ts-s", Some("sid")),
            ("tl", Some("lang")),
            ("ref", Some("loc")),
            ("fig", None),
            ("em", None),
        ] {
            assert_eq!(sheet.default_attribute(marker), name, "\\{marker}");
        }
        assert!(!sheet.get_rule_by_marker("w").unwrap().attributes.is_empty());
    }
}
