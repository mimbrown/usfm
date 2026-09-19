use notify::event::ModifyKind;
use notify::{Event, EventKind, RecursiveMode, Watcher};
use regex::Matches;
use std::borrow::Cow;
use std::fmt::{Display, Formatter};
use std::fs::canonicalize;
use std::io::Write;
use std::path::{Path, PathBuf, absolute};
use std::sync::{Arc, LazyLock, mpsc};
use usfm_ast::{Block, Document, Inline};
use usfm_parser::context::Context;
use usfm_parser::diagnostics::{ParseResult, Severity};
use usfm_parser::lexer::span::LineIndex;
use usfm_parser::serialize_html::HtmlElement;
use usfm_parser::{
    DEFAULT_STYLESHEET,
    parser::Parser,
    serialize_html::SerializeHtml,
    text_replacements::TextReplacement,
    usx::{to_usx_node, to_usx_string},
    xml_document::XmlNode,
};
use usfm_parser::{ToHtml, serialize_html};
use usfm_style::StyleSheet;

enum OutputFormat {
    Usx,
    Sile,
    Html,
    Prompt,
}

/// SILE reads the USX tree under a `<sile>` root with no attributes.
fn to_sile_string(document: &Document) -> String {
    let mut node = to_usx_node(document);
    if let XmlNode::Element(root) = &mut node {
        root.name.local_name = "sile".to_string();
        root.attributes.clear();
    }
    format!("{node}\n")
}

struct HtmlSerializer;

impl SerializeHtml for HtmlSerializer {}

enum Side {
    Left,
    Right,
}

struct IterSections<'a> {
    cursor: usize,
    text: &'a str,
    matches: Matches<'a, 'a>,
}

impl<'a> IterSections<'a> {
    fn is_opening_quote(&self, cursor: usize) -> bool {
        if !matches!(
            self.text[cursor..].chars().next(),
            Some('"' | '\'' | '”' | '“' | '‘' | '’' | '(')
        ) {
            return false;
        }
        let last_char = self.text[..cursor].chars().next_back();
        match last_char {
            Some(ch) => ch.is_whitespace(),
            None => true,
        }
    }
}

impl<'a> Iterator for IterSections<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        match self.matches.next() {
            Some(m) => {
                let cutoff = if self.is_opening_quote(m.start()) {
                    m.start()
                } else {
                    m.end()
                };
                let result = &self.text[self.cursor..cutoff];
                self.cursor = cutoff;
                Some(result)
            }
            None => {
                if self.cursor < self.text.len() {
                    let result = &self.text[self.cursor..];
                    self.cursor = self.text.len();
                    Some(result)
                } else {
                    None
                }
            }
        }
    }
}

fn sections<'a>(text: &'a str) -> IterSections<'a> {
    IterSections {
        cursor: 0,
        text,
        matches: MATCH_PUNCTUATION.find_iter(text),
    }
}

struct DocumentSectionHtmlSerializer<'a> {
    document: &'a usfm_ast::Document<'a>,
    style_sheet: &'a usfm_style::StyleSheet,
    side: Side,
}

impl<'a> DocumentSectionHtmlSerializer<'a> {
    fn new(
        document: &'a usfm_ast::Document<'a>,
        style_sheet: &'a usfm_style::StyleSheet,
        side: Side,
    ) -> Self {
        Self {
            document,
            style_sheet,
            side,
        }
    }
}

static MATCH_PUNCTUATION: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r#"[.?!,۔؟“”"'‘’،:;\(\)]+"#).unwrap());

fn drain(
    f: &mut Formatter<'_>,
    opening: &[String],
    closing: &[String],
    accumulator: &mut String,
) -> std::fmt::Result {
    if !accumulator.is_empty() && !accumulator.chars().all(char::is_whitespace) {
        for tag in opening.iter() {
            write!(f, "{}", tag)?;
        }
        write!(f, "{}", accumulator)?;
        for tag in closing.iter().rev() {
            write!(f, "{}", tag)?;
        }
    }
    accumulator.clear();
    Ok(())
}

impl<'a> Display for DocumentSectionHtmlSerializer<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        let mut context = Context::new(self.style_sheet);

        write!(
            f,
            "<section class=\"{}\">",
            match self.side {
                Side::Left => "left",
                Side::Right => "right",
            }
        )?;

        let mut opening = Vec::with_capacity(10);
        let mut closing = Vec::with_capacity(10);
        let mut accumulator = String::with_capacity(4096);

        for block in self.document.blocks.iter() {
            match block {
                Block::Para(para)
                    if self
                        .style_sheet
                        .get_rule(para.style.index())
                        .is_nonvernacular() =>
                {
                    block.to_html(f, &mut context)?;
                }
                Block::Para(para) => {
                    let tag = para.tag(&context);
                    write!(
                        f,
                        "<div class=\"para-start\">{}</div>",
                        self.style_sheet.get_rule(para.style.index()).marker
                    )?;
                    opening.push(format!(
                        "<{tag} class=\"{}\">",
                        self.style_sheet.get_rule(para.style.index()).marker
                    ));
                    closing.push(format!("</{tag}>"));

                    for inline in para.children.iter() {
                        match inline {
                            Inline::Text(text) => {
                                let mut s = sections(text);
                                if let Some(first_section) = s.next() {
                                    accumulator.push_str(first_section);
                                }
                                for section in s {
                                    drain(f, &opening, &closing, &mut accumulator)?;
                                    accumulator.push_str(section);
                                }
                            }
                            _ => {
                                inline.to_html(&mut accumulator, &mut context)?;
                            }
                        }
                    }
                    drain(f, &opening, &closing, &mut accumulator)?;
                    opening.pop();
                    closing.pop();
                    write!(
                        f,
                        "<div class=\"para-end\">{}</div>",
                        self.style_sheet.get_rule(para.style.index()).marker
                    )?;
                }
                _ => {
                    block.to_html(f, &mut context)?;
                }
            }
        }

        write!(f, "</section>")?;

        Ok(())
    }
}

fn serialize_html_diglot(
    left: &usfm_ast::Document,
    left_style_sheet: &usfm_style::StyleSheet,
    right: &usfm_ast::Document,
    right_style_sheet: &usfm_style::StyleSheet,
) -> String {
    format!(
        "{}\n{}",
        DocumentSectionHtmlSerializer::new(left, left_style_sheet, Side::Left),
        DocumentSectionHtmlSerializer::new(right, right_style_sheet, Side::Right),
    )
}

// struct IterDocumentSections<'a> {
//     block_iter: std::slice::Iter<'a, Block<'a>>,
//     text_iter: Option<IterSections<'a>>,
// }

// impl<'a> Iterator for IterDocumentSections<'a> {
//     type Item = Cow<'a, str>;

//     fn next(&mut self) -> Option<Self::Item> {
//         if let Some(ref mut text_iter) = self.text_iter {
//             if let Some(section) = text_iter.next() {
//                 return Some(Cow::Borrowed(section));
//             }
//         }
//     }
// }

fn document_sections<'a>(document: &'a Document<'a>, styles: &StyleSheet) -> Vec<Cow<'a, str>> {
    let mut result = Vec::new();
    let mut add_if_non_empty = |text: Cow<'a, str>| {
        if !text.is_empty() && !text.chars().all(char::is_whitespace) {
            result.push(match text {
                Cow::Borrowed(text) => Cow::Borrowed(text.trim()),
                Cow::Owned(text) => Cow::Owned(text.trim().to_string()),
            });
        }
    };
    let mut last_text: Option<Cow<'a, str>> = None;
    for block in document.blocks.iter() {
        if let Block::Para(para) = block {
            if styles.get_rule(para.style.index()).is_nonvernacular() {
                continue;
            }
            for inline in para.children.iter() {
                if let Inline::Text(text) = inline {
                    let mut iter = sections(text);
                    if let Some(first_text) = iter.next() {
                        if let Some(last_text) = last_text.as_mut() {
                            last_text.to_mut().push_str(first_text);
                        } else {
                            last_text = Some(Cow::Borrowed(first_text));
                        }
                    }
                    for section in iter {
                        if let Some(last_text) = last_text.as_mut() {
                            add_if_non_empty(std::mem::replace(last_text, Cow::Borrowed(section)));
                        } else {
                            last_text = Some(Cow::Borrowed(section));
                        }
                    }
                }
            }
            if let Some(last_text) = last_text.as_mut() {
                add_if_non_empty(std::mem::take(last_text));
            }
        }
    }
    result
}

struct DocumentWeaver<'a> {
    left: &'a Document<'a>,
    left_style_sheet: &'a StyleSheet,
    right: &'a Document<'a>,
    right_style_sheet: &'a StyleSheet,
}

impl Display for DocumentWeaver<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        let left_sections = document_sections(self.left, self.left_style_sheet);
        let right_sections = document_sections(self.right, self.right_style_sheet);
        for (left, right) in left_sections.iter().zip(right_sections.iter()) {
            write!(
                f,
                "<lang dialect=\"a\">{}</lang>\n<lang dialect=\"b\">{}</lang>\n\n",
                left, right
            )?;
        }
        Ok(())
    }
}

struct ParseAndTransform {
    replacements: Vec<TextReplacement>,
    replacement_paths: Vec<PathBuf>,
    style_sheet: Option<Arc<StyleSheet>>,
    style_sheet_path: Option<PathBuf>,
    input_paths: Vec<PathBuf>,
    inputs: Vec<String>,
    diglot_paths: Vec<PathBuf>,
    diglots: Vec<String>,
    diglot_style_sheet: Option<Arc<StyleSheet>>,
    diglot_style_sheet_path: Option<PathBuf>,
    diglot_replacements: Vec<TextReplacement>,
    diglot_replacement_paths: Vec<PathBuf>,
    format: OutputFormat,
    /// Refuse to produce output if parsing reported any error.
    strict: bool,
}

/// Print diagnostics to stderr as `label:line:col: severity[code]: message`.
/// Returns an error in strict mode if any diagnostic is an error.
fn report_diagnostics(
    label: &str,
    source: &str,
    result: &ParseResult,
    strict: bool,
) -> Result<(), String> {
    // One index for the whole file: the alternative is a scan from the start
    // of the source per diagnostic.
    let index = LineIndex::new(source);
    for diagnostic in &result.diagnostics {
        let (line, col) = index.line_col(diagnostic.span.start);
        eprintln!(
            "{label}:{line}:{col}: {}[{}]: {}",
            diagnostic.severity, diagnostic.code, diagnostic.message
        );
    }
    let errors = result.diagnostics_at_least(Severity::Error).count();
    if strict && errors > 0 {
        return Err(format!(
            "{label}: {errors} error(s); no output produced (--strict)"
        ));
    }
    Ok(())
}

impl ParseAndTransform {
    // Each argument is one CLI flag's worth of input; grouping them into a
    // struct is the job of the clap rewrite in M3, not of this ticket.
    #[allow(clippy::too_many_arguments)]
    fn new(
        replacement_paths: Vec<PathBuf>,
        style_sheet_path: Option<PathBuf>,
        input_paths: Vec<PathBuf>,
        diglot_paths: Vec<PathBuf>,
        diglot_style_sheet_path: Option<PathBuf>,
        diglot_replacement_paths: Vec<PathBuf>,
        format: OutputFormat,
        strict: bool,
    ) -> Result<Self, Box<(Self, Vec<Error>)>> {
        let mut errors: Vec<Error> = Vec::new();
        let replacements = replacement_paths
            .iter()
            .map(|path| match std::fs::read_to_string(path) {
                Ok(contents) => TextReplacement::from_rules(&contents),
                Err(e) => {
                    errors.push(Error::from(e));
                    TextReplacement::new()
                }
            })
            .collect::<Vec<_>>();

        let style_sheet = match style_sheet_path {
            Some(ref path) => match StyleSheet::from_file(path) {
                Ok(sheet) => Some(Arc::new(sheet)),
                Err(e) => {
                    errors.push(Error::from(e));
                    None
                }
            },
            None => None,
        };

        let inputs = input_paths
            .iter()
            .map(|path| match std::fs::read_to_string(path) {
                Ok(contents) => contents,
                Err(e) => {
                    errors.push(Error::from(e));
                    "".to_string()
                }
            })
            .collect::<Vec<_>>();

        let diglots = diglot_paths
            .iter()
            .map(|path| match std::fs::read_to_string(path) {
                Ok(contents) => contents,
                Err(e) => {
                    errors.push(Error::from(e));
                    "".to_string()
                }
            })
            .collect::<Vec<_>>();

        let diglot_style_sheet = match diglot_style_sheet_path {
            Some(ref path) => match StyleSheet::from_file(path) {
                Ok(sheet) => Some(Arc::new(sheet)),
                Err(e) => {
                    errors.push(Error::from(e));
                    None
                }
            },
            None => None,
        };

        let diglot_replacements = diglot_replacement_paths
            .iter()
            .map(|path| match std::fs::read_to_string(path) {
                Ok(contents) => TextReplacement::from_rules(&contents),
                Err(e) => {
                    errors.push(Error::from(e));
                    TextReplacement::new()
                }
            })
            .collect::<Vec<_>>();

        let instance = Self {
            replacements,
            replacement_paths,
            style_sheet,
            style_sheet_path,
            inputs,
            input_paths,
            diglots,
            diglot_paths,
            diglot_style_sheet,
            diglot_style_sheet_path,
            diglot_replacements,
            diglot_replacement_paths,
            format,
            strict,
        };
        if errors.is_empty() {
            Ok(instance)
        } else {
            // Boxed: the pair is larger than clippy's `result_large_err`
            // threshold and this is a once-per-run path.
            Err(Box::new((instance, errors)))
        }
    }

    fn paths_updated(&mut self, updated_paths: &[std::path::PathBuf]) -> Result<(), Vec<Error>> {
        let mut errors: Vec<Error> = Vec::new();

        let replacements = std::mem::take(&mut self.replacements);
        // Update any text replacements that were updated
        self.replacements = replacements
            .into_iter()
            .enumerate()
            .map(|(index, replacements)| {
                let path = &self.replacement_paths[index];
                if updated_paths.contains(path) {
                    match std::fs::read_to_string(path) {
                        Ok(contents) => TextReplacement::from_rules(&contents),
                        Err(e) => {
                            errors.push(Error::from(e));
                            TextReplacement::new()
                        }
                    }
                } else {
                    replacements
                }
            })
            .collect::<Vec<_>>();

        // Check if style sheet was updated
        if let Some(ref style_path) = self.style_sheet_path
            && updated_paths.contains(style_path)
        {
            self.style_sheet = match StyleSheet::from_file(style_path) {
                Ok(sheet) => Some(Arc::new(sheet)),
                Err(e) => {
                    errors.push(Error::from(e));
                    None
                }
            };
        }

        // Check if any inputs were updated
        let inputs = std::mem::take(&mut self.inputs);
        self.inputs = inputs
            .into_iter()
            .enumerate()
            .map(|(index, input)| {
                let path = &self.input_paths[index];
                if updated_paths.contains(path) {
                    match std::fs::read_to_string(path) {
                        Ok(contents) => contents,
                        Err(e) => {
                            errors.push(Error::from(e));
                            "".to_string()
                        }
                    }
                } else {
                    input
                }
            })
            .collect::<Vec<_>>();

        let diglots = std::mem::take(&mut self.diglots);
        self.diglots = diglots
            .into_iter()
            .enumerate()
            .map(|(index, diglot)| {
                let path = &self.diglot_paths[index];
                if updated_paths.contains(path) {
                    match std::fs::read_to_string(path) {
                        Ok(contents) => contents,
                        Err(e) => {
                            errors.push(Error::from(e));
                            "".to_string()
                        }
                    }
                } else {
                    diglot
                }
            })
            .collect::<Vec<_>>();

        if let Some(ref style_path) = self.diglot_style_sheet_path
            && updated_paths.contains(style_path)
        {
            self.diglot_style_sheet = match StyleSheet::from_file(style_path) {
                Ok(sheet) => Some(Arc::new(sheet)),
                Err(e) => {
                    errors.push(Error::from(e));
                    None
                }
            };
        }

        let diglot_replacements = std::mem::take(&mut self.diglot_replacements);
        self.diglot_replacements = diglot_replacements
            .into_iter()
            .enumerate()
            .map(|(index, replacements)| {
                let path = &self.diglot_replacement_paths[index];
                if updated_paths.contains(path) {
                    match std::fs::read_to_string(path) {
                        Ok(contents) => TextReplacement::from_rules(&contents),
                        Err(e) => {
                            errors.push(Error::from(e));
                            TextReplacement::new()
                        }
                    }
                } else {
                    replacements
                }
            })
            .collect::<Vec<_>>();

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    fn get_all_paths(&self) -> Vec<PathBuf> {
        let mut paths = Vec::new();
        paths.extend(self.input_paths.clone());
        paths.extend(self.replacement_paths.clone());
        if let Some(ref style_path) = self.style_sheet_path {
            paths.push(style_path.clone());
        }
        paths.extend(self.diglot_paths.clone());
        paths.extend(self.diglot_replacement_paths.clone());
        if let Some(ref style_path) = self.diglot_style_sheet_path {
            paths.push(style_path.clone());
        }
        paths
    }

    fn parse_and_transform(&mut self) -> Result<String, String> {
        let base_style_sheet = self
            .style_sheet
            .clone()
            .unwrap_or_else(|| Arc::clone(&DEFAULT_STYLESHEET));
        let input = self.inputs.join("\n");
        let result = Parser::new(&input).parse(&base_style_sheet);
        report_diagnostics("input", &input, &result, self.strict)?;
        let mut document = result.document;
        // Everything downstream resolves styles against the document's own
        // stylesheet, not the one handed to the parser: they differ whenever
        // the parser had to derive a style (hardening plan D3).
        let style_sheet = Arc::clone(document.style_sheet());
        for replacement in self.replacements.iter_mut() {
            replacement.apply_to(&mut document);
        }
        if !self.diglots.is_empty() {
            Ok(match self.format {
                OutputFormat::Usx => todo!(),
                OutputFormat::Sile => todo!(),
                OutputFormat::Html => {
                    let base_diglot_style_sheet = self
                        .diglot_style_sheet
                        .clone()
                        .unwrap_or_else(|| Arc::clone(&DEFAULT_STYLESHEET));
                    let diglot = self.diglots.join("\n");
                    let result = Parser::new(&diglot).parse(&base_diglot_style_sheet);
                    report_diagnostics("diglot", &diglot, &result, self.strict)?;
                    let mut diglot_document = result.document;
                    let diglot_style_sheet = Arc::clone(diglot_document.style_sheet());
                    for replacement in self.diglot_replacements.iter_mut() {
                        replacement.apply_to(&mut diglot_document);
                    }
                    serialize_html_diglot(
                        &document,
                        &style_sheet,
                        &diglot_document,
                        &diglot_style_sheet,
                    )
                }
                OutputFormat::Prompt => {
                    let base_diglot_style_sheet = self
                        .diglot_style_sheet
                        .clone()
                        .unwrap_or_else(|| Arc::clone(&DEFAULT_STYLESHEET));
                    let diglot = self.diglots.join("\n");
                    let result = Parser::new(&diglot).parse(&base_diglot_style_sheet);
                    report_diagnostics("diglot", &diglot, &result, self.strict)?;
                    let mut diglot_document = result.document;
                    let diglot_style_sheet = Arc::clone(diglot_document.style_sheet());
                    for replacement in self.diglot_replacements.iter_mut() {
                        replacement.apply_to(&mut diglot_document);
                    }
                    format!(
                        "{}",
                        DocumentWeaver {
                            left: &document,
                            left_style_sheet: &style_sheet,
                            right: &diglot_document,
                            right_style_sheet: &diglot_style_sheet,
                        }
                    )
                }
            })
        } else {
            Ok(match self.format {
                OutputFormat::Usx => to_usx_string(&document),
                OutputFormat::Sile => to_sile_string(&document),
                OutputFormat::Html => serialize_html(&document, &style_sheet, HtmlSerializer),
                OutputFormat::Prompt => unimplemented!(),
            })
        }
    }

    fn run(&mut self, output: &Option<PathBuf>) -> Result<(), Error> {
        let content = self.parse_and_transform()?;
        match output {
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
        };
        Ok(())
    }
}

#[derive(Debug)]
enum Error {
    Io(std::io::Error),
    Custom(String),
    Consumed,
}

impl Error {
    fn print(&self) {
        match self {
            Error::Io(e) => {
                eprintln!("IO Error: {}", e);
            }
            Error::Custom(e) => {
                eprintln!("Error: {}", e);
            }
            Error::Consumed => {}
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl<'a> From<&'a str> for Error {
    fn from(value: &'a str) -> Self {
        Self::Custom(value.to_string())
    }
}

impl From<String> for Error {
    fn from(value: String) -> Self {
        Self::Custom(value)
    }
}

fn get_arg(arg: &str, args: &mut impl Iterator<Item = String>) -> Result<String, Error> {
    args.next()
        .ok_or_else(|| Error::Custom(format!("Expected argument for {}", arg)))
}

fn get_path(
    arg: &str,
    args: &mut impl Iterator<Item = String>,
    expect_existing: bool,
) -> Result<PathBuf, Error> {
    let path = get_arg(arg, args)?;
    if expect_existing {
        std::fs::canonicalize(path).map_err(Error::from)
    } else {
        absolute(path).map_err(Error::from)
    }
}

fn run() -> Result<(), Error> {
    let mut inputs = Vec::new();
    let mut diglots = Vec::new();
    let mut format: OutputFormat = OutputFormat::Usx;
    let mut output = None;
    let mut replacements = Vec::new();
    let mut style_sheet: Option<PathBuf> = None;
    let mut diglot_style_sheet: Option<PathBuf> = None;
    let mut diglot_replacements = Vec::new();
    let mut args = std::env::args().skip(1);
    let mut watch = false;
    let mut strict = false;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--format" | "-f" => {
                format = match get_arg(&arg, &mut args)?.to_lowercase().as_str() {
                    "usx" => OutputFormat::Usx,
                    "sile" => OutputFormat::Sile,
                    "html" => OutputFormat::Html,
                    "prompt" => OutputFormat::Prompt,
                    // "usfm" => OutputFormat::Usfm,
                    _ => return Err("Invalid format".into()),
                };
            }
            "--style" | "-s" => {
                style_sheet = Some(get_path(&arg, &mut args, true)?);
            }
            "--output" | "-o" => {
                output = Some(get_path(&arg, &mut args, false)?);
            }
            "--replacements" | "-r" => {
                replacements.push(get_path(&arg, &mut args, true)?);
            }
            "--watch" | "-w" => {
                watch = true;
            }
            "--strict" => {
                strict = true;
            }
            "--diglot" | "-d" => {
                diglots.push(get_path(&arg, &mut args, true)?);
            }
            "--diglot-style" | "-ds" => {
                diglot_style_sheet = Some(get_path(&arg, &mut args, true)?);
            }
            "--diglot-replacements" | "-dr" => {
                diglot_replacements.push(get_path(&arg, &mut args, true)?);
            }
            _ => inputs.push(canonicalize(&arg)?),
        }
    }

    let parse_and_transform_result = ParseAndTransform::new(
        replacements,
        style_sheet,
        inputs,
        diglots,
        diglot_style_sheet,
        diglot_replacements,
        format,
        strict,
    );

    if watch {
        let mut parse_and_transform = match parse_and_transform_result {
            Ok(parse_and_transform) => parse_and_transform,
            Err(failed) => {
                let (parse_and_transform, errors) = *failed;
                for error in errors {
                    error.print();
                }
                parse_and_transform
            }
        };
        let (tx, rx) = mpsc::channel::<notify::Result<Event>>();
        let mut watcher = notify::recommended_watcher(tx).map_err(std::io::Error::other)?;

        // Watch all relevant files
        for path_str in parse_and_transform.get_all_paths() {
            if let Ok(path) = Path::new(&path_str).canonicalize() {
                watcher
                    .watch(&path, RecursiveMode::NonRecursive)
                    .map_err(std::io::Error::other)?;
            }
        }

        // Process files initially
        if let Err(e) = parse_and_transform.run(&output) {
            e.print();
        }

        if output.is_some() {
            println!("Watching for changes... Press Ctrl+C to exit");
        }

        // Watch for changes
        for res in rx {
            match res {
                Ok(event) => {
                    if matches!(
                        event.kind,
                        EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_)
                    ) && !matches!(event.kind, EventKind::Modify(ModifyKind::Metadata(_)))
                    {
                        if let Err(e) = parse_and_transform.paths_updated(&event.paths) {
                            for error in e {
                                error.print();
                            }
                            continue;
                        }

                        if let Err(e) = parse_and_transform.run(&output) {
                            e.print();
                        }
                    }
                }
                Err(e) => eprintln!("Watch error: {:?}", e),
            }
        }
    } else {
        let mut parse_and_transform = match parse_and_transform_result {
            Ok(parse_and_transform) => parse_and_transform,
            Err(failed) => {
                let (_, errors) = *failed;
                for error in errors {
                    error.print();
                }
                return Err(Error::Consumed);
            }
        };
        parse_and_transform.run(&output)?;
    }

    Ok(())
}

fn main() {
    match run() {
        Ok(()) => {}
        Err(e) => {
            e.print();
            std::process::exit(1);
        }
    }
}
