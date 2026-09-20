//! Whether a marker may stand under another: `\OccursUnder`, read as the rule
//! ticket 20 reports with.
//!
//! [`check`] is the whole of that rule, as a function of the stylesheet, the
//! marker's own entry and the marker it would sit under. `check_placement` in
//! this crate is that function plus the two things a *check* needs on top of
//! it — which parent the tree says is in force, and a memo so the answer is
//! worked out once per pair (ticket 24) — and nothing else reads the
//! `OccursUnder` list.
//!
//! It is public because a second caller wants the same question answered the
//! other way round. The language server's completion (ticket 32) has a parent
//! and the whole sheet, and asks of every marker whether it may be offered
//! there; the check has one marker and asks whether it should be reported. One
//! answer, so one implementation: a marker the server offers is never one the
//! checks would call an error, and a rule changed here changes both at once.
//! Nothing in this module knows anything about an editor — it takes a sheet,
//! a rule and a marker name, and this crate depends on no server.

use usfm_style::{StyleRule, StyleSheet};

/// What `\OccursUnder` says about one marker under one parent.
///
/// The two codes of ticket 20 are the two ways of not being [`Listed`]: a
/// marker whose whole list is note styles (`\fq`, `\xo`, `\fr` …) exists only
/// inside a note, so anywhere else it is an Error and Paratext marks it
/// `status="invalid"`; every other list is advisory — Paratext accepts `\f`
/// under `\cl`, which `\f` does not list — so it only informs.
///
/// [`Listed`]: Placement::Listed
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Placement {
    /// The parent is in the marker's `\OccursUnder`, or the list is empty,
    /// which is what an unrestricted marker has.
    Listed,
    /// Not listed, but the list is not note-only: `marker-not-listed-here`
    /// (Info).
    NotListed,
    /// Not listed, and every marker the list does name is a note style, so
    /// this marker exists only inside a note: `marker-not-allowed-here`
    /// (Error).
    NotAllowed,
}

impl Placement {
    /// Whether the marker may stand there at all: everything but
    /// [`NotAllowed`](Placement::NotAllowed).
    ///
    /// The line the language server's completion filters on, and the same
    /// line the checks draw between an Error and an Info.
    pub fn is_allowed(self) -> bool {
        self != Placement::NotAllowed
    }
}

/// `rule` under the marker named `parent`, by its `\OccursUnder` list.
///
/// `sheet` is only read to find out whether the markers in the list are note
/// styles, which is what separates the two ways of not being listed.
pub fn check(sheet: &StyleSheet, rule: &StyleRule, parent: &str) -> Placement {
    if rule.occurs_under.is_empty() {
        return Placement::Listed;
    }
    if rule.occurs_under.iter().any(|allowed| allowed == parent) {
        return Placement::Listed;
    }
    let note_only = rule.occurs_under.iter().all(|allowed| {
        sheet
            .get_rule_by_marker(allowed)
            .is_some_and(|allowed| allowed.is_note())
    });
    if note_only {
        Placement::NotAllowed
    } else {
        Placement::NotListed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The facade, a dev-dependency of this crate (see `Cargo.toml`): the
    // sheet these markers are read from is the one the parser builds.
    use usfm::DEFAULT_STYLESHEET;

    fn placement(marker: &str, parent: &str) -> Placement {
        let rule = DEFAULT_STYLESHEET
            .get_rule_by_marker(marker)
            .unwrap_or_else(|| panic!("`\\{marker}` is in the default sheet"));
        check(&DEFAULT_STYLESHEET, rule, parent)
    }

    /// The three answers, on the markers the checks' own tests use.
    #[test]
    fn the_three_verdicts() {
        // `\fq` occurs under `\f`, `\fe` and `\ef`, which are all notes.
        assert_eq!(placement("fq", "f"), Placement::Listed);
        assert_eq!(placement("fq", "p"), Placement::NotAllowed);
        assert!(!placement("fq", "p").is_allowed());

        // `\f` lists paragraph markers among its parents, so a parent it does
        // not list is advisory only.
        assert_eq!(placement("f", "p"), Placement::Listed);
        assert_eq!(placement("f", "cl"), Placement::NotListed);
        assert!(placement("f", "cl").is_allowed());

        // A marker with no `\OccursUnder` at all — which is what the parser
        // derives for an unknown milestone (`\zaln-s`) — stands anywhere.
        let unrestricted = StyleRule {
            marker: "zaln-s".to_owned(),
            name: None,
            description: None,
            style_type: usfm_style::StyleType::Milestone,
            text_type: usfm_style::TextType::Other,
            text_properties: usfm_style::TextProperties::default(),
            nest: false,
            occurs_under: Vec::new(),
        };
        assert_eq!(
            check(&DEFAULT_STYLESHEET, &unrestricted, "p"),
            Placement::Listed,
        );
    }
}
