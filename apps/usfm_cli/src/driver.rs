//! Reading the files a run needs, parsing them, and writing the output.
//!
//! The `ParseAndTransform` of the old `usfm_parser/src/main.rs`, with the
//! transforms themselves now in `usfm_pipeline`. What is left here is the part that
//! touches the filesystem: which files were read, so watch mode can read them
//! again, and where the result goes.
//!
//! `--from` (ticket 48) decides how the files become a document. USFM files
//! are concatenated and parsed as one source, since a book split across files
//! is still one book. USX files cannot be: two USX files side by side are not
//! one XML document (each has its own `<usx>` root), so each is read on its
//! own, reports its diagnostics under its own path with positions in its own
//! file, and the documents are then combined block by block, in the order
//! given ([`combine`]). That is what concatenation does for USFM, whose files
//! are one book each in practice; a USX file always is.

use std::io::Write;
use std::path::{Path, PathBuf, absolute};
use std::sync::Arc;

use usfm::ast::visit_mut::{self, VisitMut};
use usfm::ast::{Char, Document, Milestone, Note, Para, Periph, Sidebar, StyleId};
use usfm::diagnostics::{ParseResult, Severity};
use usfm::parser::DEFAULT_STYLESHEET;
use usfm::pipeline::{OutputFormat, TextReplacement};
use usfm::span::LineIndex;
use usfm::style::StyleSheet;

use crate::args::{DiagnosticFormat, InputFormat, ParseArgs};
use crate::error::Error;

/// A file that was read, kept with the path it came from so watch mode can
/// read it again. A file that could not be read leaves `value` at its default
/// (an empty source, no rules, no stylesheet) and the failure in the caller's
/// error list, so one missing file does not take the whole run down.
struct Loaded<T> {
    path: PathBuf,
    value: T,
}

impl<T: Default> Loaded<T> {
    fn new(
        path: PathBuf,
        read: impl Fn(&Path) -> Result<T, Error>,
        errors: &mut Vec<Error>,
    ) -> Self {
        let value = match read(&path) {
            Ok(value) => value,
            Err(e) => {
                errors.push(e);
                T::default()
            }
        };
        Self { path, value }
    }

    /// Read the file again if it is one of `updated`.
    fn reload(
        &mut self,
        updated: &[PathBuf],
        read: impl Fn(&Path) -> Result<T, Error>,
        errors: &mut Vec<Error>,
    ) {
        if !updated.contains(&self.path) {
            return;
        }
        match read(&self.path) {
            Ok(value) => self.value = value,
            Err(e) => {
                errors.push(e);
                self.value = T::default();
            }
        }
    }
}

/// The bytes of one file, as a string. Shared with [`crate::format`], which
/// reads each of its files the same way.
pub fn read_source(path: &Path) -> Result<String, Error> {
    std::fs::read_to_string(path).map_err(Error::from)
}

fn read_rules(path: &Path) -> Result<TextReplacement, Error> {
    Ok(TextReplacement::from_rules(&read_source(path)?))
}

/// A `--stylesheet` file, if one was named. The `Option` is what [`Loaded`]
/// needs (a failed read leaves `None`); [`crate::format`] reads its own
/// `--stylesheet` through the same function so the two commands accept the
/// same sheets.
pub fn read_stylesheet(path: &Path) -> Result<Option<Arc<StyleSheet>>, Error> {
    StyleSheet::from_file(path)
        .map(|sheet| Some(Arc::new(sheet)))
        .map_err(Error::from)
}

/// The path a flag named, resolved so that a watch event's path can be
/// compared against it. An input file has to exist; the output file does not
/// yet.
fn existing(path: &Path) -> Result<PathBuf, Error> {
    std::fs::canonicalize(path).map_err(|e| Error::Custom(format!("{}: {}", path.display(), e)))
}

/// How diagnostics are printed, and what makes them fatal.
pub struct DiagnosticOptions {
    format: DiagnosticFormat,
    threshold: Option<Severity>,
    flag: &'static str,
}

/// Print every diagnostic to standard error, `label` being the name the lines
/// are filed under. Shared with [`crate::format`], which prints the same lines
/// per file.
pub fn print_diagnostics(
    label: &str,
    source: &str,
    diagnostics: &[usfm::Diagnostic],
    format: DiagnosticFormat,
) {
    // One index for the whole file: the alternative is a scan from the start
    // of the source per diagnostic.
    let index = LineIndex::new(source);
    for diagnostic in diagnostics {
        // The lines themselves are `usfm_diagnostics`' (ticket 12), so the CLI
        // and the language server position and name things the same way.
        match format {
            DiagnosticFormat::Text => eprintln!("{}", diagnostic.render(label, &index)),
            DiagnosticFormat::Json => eprintln!("{}", diagnostic.to_json_line(label, &index)),
        }
    }
}

/// Print every diagnostic to stderr and hand back the document, unless the
/// run has a threshold and something reached it.
fn report<'a>(
    label: &str,
    source: &str,
    result: ParseResult<'a>,
    options: &DiagnosticOptions,
) -> Result<Document<'a>, Error> {
    print_diagnostics(label, source, &result.diagnostics, options.format);
    enforce(label, result, options)
}

/// The document, unless the run has a threshold and one of `result`'s
/// diagnostics reached it.
fn enforce<'a>(
    label: &str,
    result: ParseResult<'a>,
    options: &DiagnosticOptions,
) -> Result<Document<'a>, Error> {
    match options.threshold {
        None => Ok(result.document),
        Some(threshold) => result.strict_with(threshold).map_err(|diagnostics| {
            let count = diagnostics
                .iter()
                .filter(|d| d.severity >= threshold)
                .count();
            Error::Custom(format!(
                "{label}: {count} diagnostic(s) at {threshold} or above; no output produced ({})",
                options.flag
            ))
        }),
    }
}

/// One run's worth of files, formats and policy.
pub struct Driver {
    inputs: Vec<Loaded<String>>,
    style_sheet: Option<Loaded<Option<Arc<StyleSheet>>>>,
    replacements: Vec<Loaded<TextReplacement>>,
    diglots: Vec<Loaded<String>>,
    diglot_style_sheet: Option<Loaded<Option<Arc<StyleSheet>>>>,
    diglot_replacements: Vec<Loaded<TextReplacement>>,
    from: InputFormat,
    format: OutputFormat,
    output: Option<PathBuf>,
    diagnostics: DiagnosticOptions,
}

impl Driver {
    /// Read everything `args` names. Any file that could not be read is in the
    /// returned error list; the driver is usable either way, which is what
    /// watch mode needs — the missing file may be about to appear.
    pub fn new(args: &ParseArgs) -> (Self, Vec<Error>) {
        let mut errors = Vec::new();
        let files = resolve_all(&args.files, &mut errors);
        let replace = resolve_all(&args.replace, &mut errors);
        let diglot = resolve_all(&args.diglot, &mut errors);
        let diglot_replace = resolve_all(&args.diglot_replace, &mut errors);
        let stylesheet = resolve_one(args.stylesheet.as_deref(), &mut errors);
        let diglot_stylesheet = resolve_one(args.diglot_stylesheet.as_deref(), &mut errors);

        let output = match args.output.as_deref().map(absolute) {
            Some(Ok(path)) => Some(path),
            Some(Err(e)) => {
                errors.push(Error::from(e));
                None
            }
            None => None,
        };

        let driver = Self {
            inputs: files
                .into_iter()
                .map(|path| Loaded::new(path, read_source, &mut errors))
                .collect(),
            style_sheet: stylesheet.map(|path| Loaded::new(path, read_stylesheet, &mut errors)),
            replacements: replace
                .into_iter()
                .map(|path| Loaded::new(path, read_rules, &mut errors))
                .collect(),
            diglots: diglot
                .into_iter()
                .map(|path| Loaded::new(path, read_source, &mut errors))
                .collect(),
            diglot_style_sheet: diglot_stylesheet
                .map(|path| Loaded::new(path, read_stylesheet, &mut errors)),
            diglot_replacements: diglot_replace
                .into_iter()
                .map(|path| Loaded::new(path, read_rules, &mut errors))
                .collect(),
            from: args.from,
            format: args.format.into(),
            output,
            diagnostics: DiagnosticOptions {
                format: args.diagnostics,
                threshold: args.threshold(),
                flag: args.threshold_flag(),
            },
        };
        (driver, errors)
    }

    /// Reread whichever of this run's files are in `updated`.
    pub fn paths_updated(&mut self, updated: &[PathBuf]) -> Result<(), Vec<Error>> {
        let mut errors = Vec::new();
        for input in self.inputs.iter_mut().chain(self.diglots.iter_mut()) {
            input.reload(updated, read_source, &mut errors);
        }
        for rules in self
            .replacements
            .iter_mut()
            .chain(self.diglot_replacements.iter_mut())
        {
            rules.reload(updated, read_rules, &mut errors);
        }
        for sheet in self
            .style_sheet
            .iter_mut()
            .chain(self.diglot_style_sheet.iter_mut())
        {
            sheet.reload(updated, read_stylesheet, &mut errors);
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Every file this run reads, which is what watch mode watches.
    pub fn watched_paths(&self) -> Vec<PathBuf> {
        let mut paths: Vec<PathBuf> = Vec::new();
        paths.extend(self.inputs.iter().map(|f| f.path.clone()));
        paths.extend(self.replacements.iter().map(|f| f.path.clone()));
        paths.extend(self.style_sheet.iter().map(|f| f.path.clone()));
        paths.extend(self.diglots.iter().map(|f| f.path.clone()));
        paths.extend(self.diglot_replacements.iter().map(|f| f.path.clone()));
        paths.extend(self.diglot_style_sheet.iter().map(|f| f.path.clone()));
        paths
    }

    /// True if the output goes to a file rather than to standard output.
    pub fn writes_to_file(&self) -> bool {
        self.output.is_some()
    }

    /// Parse, transform and render, without writing anything.
    pub fn render(&mut self) -> Result<String, Error> {
        let mut input = String::new();
        let mut document = read(
            self.from,
            &self.inputs,
            "input",
            &sheet_or_default(&self.style_sheet),
            &self.diagnostics,
            &mut input,
        )?;
        // Everything downstream resolves styles against the document's own
        // stylesheet, not the one handed to the parser: they differ whenever
        // the parser had to derive a style (hardening plan D3).
        let style_sheet = Arc::clone(document.style_sheet());
        for replacement in self.replacements.iter_mut() {
            replacement.value.apply_to(&mut document);
        }

        if self.diglots.is_empty() {
            return Ok(usfm::pipeline::render(&document, &style_sheet, self.format)?);
        }

        // Checked before the second file is parsed, so `--format usx --diglot`
        // does not print a page of diagnostics for a document it will not use.
        if !self.format.supports_diglot() {
            return Err(usfm::pipeline::RenderError::NoDiglotForm(self.format).into());
        }
        let mut diglot = String::new();
        let mut diglot_document = read(
            self.from,
            &self.diglots,
            "diglot",
            &sheet_or_default(&self.diglot_style_sheet),
            &self.diagnostics,
            &mut diglot,
        )?;
        let diglot_style_sheet = Arc::clone(diglot_document.style_sheet());
        for replacement in self.diglot_replacements.iter_mut() {
            replacement.value.apply_to(&mut diglot_document);
        }
        Ok(usfm::pipeline::render_diglot(
            &document,
            &style_sheet,
            &diglot_document,
            &diglot_style_sheet,
            self.format,
        )?)
    }

    /// Render and write, to `--output` or to standard output.
    pub fn run(&mut self) -> Result<(), Error> {
        let content = self.render()?;
        match &self.output {
            Some(file) => std::fs::write(file, content)?,
            None => {
                let mut bytes = content.as_bytes();
                loop {
                    match std::io::stdout().write(bytes) {
                        Ok(0) => break,
                        Ok(n) => bytes = &bytes[n..],
                        Err(e) => {
                            if e.kind() == std::io::ErrorKind::Interrupted {
                                continue;
                            }
                            return Err(Error::from(e));
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

/// Resolve a list of paths, recording each failure rather than returning it:
/// a run reports every unreadable file, not just the first.
fn resolve_all(paths: &[PathBuf], errors: &mut Vec<Error>) -> Vec<PathBuf> {
    paths
        .iter()
        .filter_map(|path| resolve_one(Some(path.as_path()), errors))
        .collect()
}

/// Resolve one optional path the same way.
fn resolve_one(path: Option<&Path>, errors: &mut Vec<Error>) -> Option<PathBuf> {
    match existing(path?) {
        Ok(path) => Some(path),
        Err(e) => {
            errors.push(e);
            None
        }
    }
}

/// The files of one side, as one document, with their diagnostics printed.
///
/// USFM is [joined](join) into `joined` — which the document then borrows —
/// and parsed as one source, its diagnostics filed under `label`. USX is read
/// file by file, each file's diagnostics filed under its own path, and the
/// documents [combined](combine); `joined` stays empty. Either way through the
/// facade, not `Parser` or `read_usx` directly: `--strict`, `--deny-warnings`
/// and `--diagnostics json` should see the semantic checks as well as the
/// repairs (ticket 19).
fn read<'a>(
    from: InputFormat,
    files: &'a [Loaded<String>],
    label: &str,
    style_sheet: &Arc<StyleSheet>,
    options: &DiagnosticOptions,
    joined: &'a mut String,
) -> Result<Document<'a>, Error> {
    match from {
        InputFormat::Usfm => {
            *joined = join(files);
            let joined: &'a str = joined;
            report(
                label,
                joined,
                usfm::parse_with(joined, style_sheet),
                options,
            )
        }
        InputFormat::Usx => {
            // Every file's diagnostics are printed before any threshold
            // stops the run, so one refused file does not hide what the
            // others report.
            let mut documents = Vec::with_capacity(files.len());
            let mut refused = Vec::new();
            for file in files {
                let label = file.path.display().to_string();
                let result = usfm::parse_usx_with(&file.value, style_sheet);
                print_diagnostics(&label, &file.value, &result.diagnostics, options.format);
                match enforce(&label, result, options) {
                    Ok(document) => documents.push(document),
                    Err(e) => refused.push(e),
                }
            }
            match refused.len() {
                0 => Ok(combine(documents, style_sheet)),
                1 => Err(refused.remove(0)),
                _ => {
                    for e in &refused {
                        e.print();
                    }
                    Err(Error::Consumed)
                }
            }
        }
    }
}

/// Several documents as one, their blocks in order: what concatenating their
/// sources would have made, for documents that cannot be concatenated as
/// text (USX, see the module docs).
///
/// Every document was read against `style_sheet`, so a `StyleId` below its
/// length means the same style in all of them. Above it is a style that one
/// document derived onto its own copy of the sheet (plan D3), and the same
/// index can name different styles in two documents; each such id is moved to
/// the combined sheet's entry for the same marker, added when no document
/// before it derived that marker too. The combined document's `span` is
/// [`SPAN`](usfm::ast::SPAN): its nodes' spans point into different files, and
/// no one range covers them. A single document is returned as it is.
fn combine<'a>(documents: Vec<Document<'a>>, style_sheet: &Arc<StyleSheet>) -> Document<'a> {
    let mut documents = documents.into_iter();
    let Some(first) = documents.next() else {
        return Document::new(Vec::new(), Arc::clone(style_sheet));
    };
    let Some(second) = documents.next() else {
        return first;
    };
    let shared = style_sheet.rules.len();
    let mut sheet = Arc::clone(first.style_sheet());
    let mut blocks = first.blocks;
    for mut document in std::iter::once(second).chain(documents) {
        let own = Arc::clone(document.style_sheet());
        if !Arc::ptr_eq(&own, &sheet) && own.rules.len() > shared {
            let ids: Vec<u32> = own
                .rules
                .iter()
                .enumerate()
                .map(|(index, rule)| {
                    let index = if index < shared {
                        index
                    } else {
                        match sheet.get_marker_index(&rule.marker) {
                            Some(&existing) if existing >= shared => existing,
                            _ => Arc::make_mut(&mut sheet).add_rule(rule.clone()),
                        }
                    };
                    index as u32
                })
                .collect();
            Restyle(&ids).visit_document(&mut document);
        }
        blocks.append(&mut document.blocks);
    }
    Document::new(blocks, sheet)
}

/// Moves every `StyleId` to `self.0[id]`, for [`combine`]. The six nodes
/// with a `style` are the six overridden here.
struct Restyle<'m>(&'m [u32]);

impl Restyle<'_> {
    fn restyle(&self, style: &mut StyleId) {
        *style = StyleId::new(self.0[style.index()]);
    }
}

impl VisitMut for Restyle<'_> {
    fn visit_para(&mut self, para: &mut Para<'_>) {
        self.restyle(&mut para.style);
        visit_mut::walk_para(self, para);
    }

    fn visit_char(&mut self, char: &mut Char<'_>) {
        self.restyle(&mut char.style);
        visit_mut::walk_char(self, char);
    }

    fn visit_note(&mut self, note: &mut Note<'_>) {
        self.restyle(&mut note.style);
        visit_mut::walk_note(self, note);
    }

    fn visit_milestone(&mut self, milestone: &mut Milestone<'_>) {
        self.restyle(&mut milestone.style);
        visit_mut::walk_milestone(self, milestone);
    }

    fn visit_sidebar(&mut self, sidebar: &mut Sidebar<'_>) {
        self.restyle(&mut sidebar.style);
        visit_mut::walk_sidebar(self, sidebar);
    }

    fn visit_periph(&mut self, periph: &mut Periph<'_>) {
        self.restyle(&mut periph.style);
        visit_mut::walk_periph(self, periph);
    }
}

/// The files of one side, as one source. They are concatenated rather than
/// parsed separately: a book split across files is still one book.
fn join(files: &[Loaded<String>]) -> String {
    files
        .iter()
        .map(|file| file.value.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

/// The stylesheet to parse against: the one `--stylesheet` named, or the
/// built-in default.
fn sheet_or_default(sheet: &Option<Loaded<Option<Arc<StyleSheet>>>>) -> Arc<StyleSheet> {
    sheet
        .as_ref()
        .and_then(|sheet| sheet.value.clone())
        .unwrap_or_else(|| Arc::clone(&DEFAULT_STYLESHEET))
}
