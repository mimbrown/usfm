//! Which crate writes which output format.
//!
//! The dispatch the CLI's driver used to hold inline, with the two arms that
//! were `todo!()` and the one that was `unimplemented!()` turned into errors
//! that say what to do instead (ticket 15). Nothing here panics on a
//! combination of flags.

use usfm_ast::Document;
use usfm_style::StyleSheet;

use crate::diglot::{serialize_html_diglot, weave_prompt};
use crate::sile::to_sile_string;

/// What the pipeline writes. `usfm_cli`'s `--format` is this list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputFormat {
    /// USX, the XML serialisation of the AST.
    Usx,
    /// HTML.
    Html,
    /// The AST as JSON, one object per node (ticket 16).
    Json,
    /// USFM again, in the writer's canonical shape (ticket 26). This is what
    /// `usfm format` writes; as a `--format` it is the way to normalise a
    /// document assembled from several files.
    Usfm,
    /// SILE's flavour of USX.
    Sile,
    /// Two translations woven section by section, for a language model.
    Prompt,
}

impl OutputFormat {
    /// Whether two documents can be written side by side in this format.
    ///
    /// [`render_diglot`] answers the same question by refusing, but a caller
    /// that would have to read and parse a second file first can ask before
    /// doing the work. The two cannot drift apart: a test below checks every
    /// variant against what `render_diglot` does with it.
    pub fn supports_diglot(self) -> bool {
        matches!(self, OutputFormat::Html | OutputFormat::Prompt)
    }

    /// The name the format is asked for by on the command line.
    pub fn name(self) -> &'static str {
        match self {
            OutputFormat::Usx => "usx",
            OutputFormat::Html => "html",
            OutputFormat::Json => "json",
            OutputFormat::Usfm => "usfm",
            OutputFormat::Sile => "sile",
            OutputFormat::Prompt => "prompt",
        }
    }
}

impl std::fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

/// A format asked for in a shape it cannot be written in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RenderError {
    /// The format is a pairing of two translations, and only one was given.
    NeedsDiglot(OutputFormat),
    /// Two translations were given, and the format has no paired form.
    NoDiglotForm(OutputFormat),
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RenderError::NeedsDiglot(format) => write!(
                f,
                "{format} output pairs two translations: pass the second one with --diglot"
            ),
            RenderError::NoDiglotForm(format) => write!(
                f,
                "{format} output has no diglot form: with --diglot the format must be html or prompt"
            ),
        }
    }
}

impl std::error::Error for RenderError {}

/// Write one document in `format`.
///
/// `style_sheet` is the document's own (`document.style_sheet()`), which is
/// the sheet handed to the parser extended with anything it had to derive
/// (hardening plan D3).
pub fn render(
    document: &Document,
    style_sheet: &StyleSheet,
    format: OutputFormat,
) -> Result<String, RenderError> {
    match format {
        OutputFormat::Usx => Ok(usfm_usx::to_usx_string(document)),
        OutputFormat::Html => Ok(usfm_html::to_html_string(document, style_sheet)),
        OutputFormat::Json => {
            // The compact form, which is what a pipe wants, with the trailing
            // newline every other format here ends with.
            let mut json = usfm_json::to_json_string(document);
            json.push('\n');
            Ok(json)
        }
        OutputFormat::Usfm => Ok(usfm_codegen::to_usfm_string(document)),
        OutputFormat::Sile => Ok(to_sile_string(document)),
        OutputFormat::Prompt => Err(RenderError::NeedsDiglot(OutputFormat::Prompt)),
    }
}

/// Write two documents side by side in `format`.
pub fn render_diglot(
    left: &Document,
    left_style_sheet: &StyleSheet,
    right: &Document,
    right_style_sheet: &StyleSheet,
    format: OutputFormat,
) -> Result<String, RenderError> {
    match format {
        OutputFormat::Html => Ok(serialize_html_diglot(
            left,
            left_style_sheet,
            right,
            right_style_sheet,
        )),
        OutputFormat::Prompt => Ok(weave_prompt(
            left,
            left_style_sheet,
            right,
            right_style_sheet,
        )),
        OutputFormat::Usx | OutputFormat::Json | OutputFormat::Usfm | OutputFormat::Sile => {
            Err(RenderError::NoDiglotForm(format))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::parse;

    /// Every single-document format writes something recognisable, and the
    /// stylesheet used is the document's own.
    #[test]
    fn each_format_writes_its_own_shape() {
        let (document, style_sheet) = parse("\\id GEN\n\\c 1\n\\p \\v 1 verse one");

        let usx = render(&document, &style_sheet, OutputFormat::Usx).unwrap();
        assert!(usx.starts_with("<usx"), "{usx}");
        let html = render(&document, &style_sheet, OutputFormat::Html).unwrap();
        assert!(html.contains("<p class=\"p\">"), "{html}");
        let sile = render(&document, &style_sheet, OutputFormat::Sile).unwrap();
        assert!(sile.starts_with("<sile>"), "{sile}");
        let json = render(&document, &style_sheet, OutputFormat::Json).unwrap();
        assert!(json.starts_with('{'), "{json}");
        assert!(json.ends_with("}\n"), "{json}");
        assert!(json.contains(r#""type":"document""#), "{json}");
        let usfm = render(&document, &style_sheet, OutputFormat::Usfm).unwrap();
        assert_eq!(usfm, "\\id GEN\n\\c 1\n\\p \\v 1 verse one\n", "{usfm}");
    }

    /// The combinations that used to be `todo!()` and `unimplemented!()` are
    /// errors that name the flag to change.
    #[test]
    fn an_impossible_combination_is_an_error_not_a_panic() {
        let (document, style_sheet) = parse("\\id GEN\n\\c 1\n\\p \\v 1 verse one");
        let (other, other_sheet) = parse("\\id GEN\n\\c 1\n\\p \\v 1 vers eins");

        assert_eq!(
            render(&document, &style_sheet, OutputFormat::Prompt),
            Err(RenderError::NeedsDiglot(OutputFormat::Prompt))
        );
        assert!(
            render(&document, &style_sheet, OutputFormat::Prompt)
                .unwrap_err()
                .to_string()
                .contains("--diglot")
        );
        for format in [
            OutputFormat::Usx,
            OutputFormat::Json,
            OutputFormat::Usfm,
            OutputFormat::Sile,
        ] {
            assert_eq!(
                render_diglot(&document, &style_sheet, &other, &other_sheet, format),
                Err(RenderError::NoDiglotForm(format))
            );
        }
    }

    /// `supports_diglot` is what `render_diglot` does, for every format.
    #[test]
    fn supports_diglot_agrees_with_render_diglot() {
        let (left, left_sheet) = parse("\\id GEN\n\\c 1\n\\p \\v 1 verse one");
        let (right, right_sheet) = parse("\\id GEN\n\\c 1\n\\p \\v 1 vers eins");
        for format in [
            OutputFormat::Usx,
            OutputFormat::Html,
            OutputFormat::Json,
            OutputFormat::Usfm,
            OutputFormat::Sile,
            OutputFormat::Prompt,
        ] {
            assert_eq!(
                format.supports_diglot(),
                render_diglot(&left, &left_sheet, &right, &right_sheet, format).is_ok(),
                "{format}"
            );
        }
    }
}
