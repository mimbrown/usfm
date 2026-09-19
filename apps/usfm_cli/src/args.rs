//! The command line, as a struct.
//!
//! Every flag the hand-rolled argument loop in the old `usfm_parser/src/main.rs`
//! understood is here, plus `--deny-warnings` and `--diagnostics` (ticket 15).
//! The long names of four flags changed with the move: `--style` is
//! `--stylesheet`, `--replacements` is `--replace`, `--diglot-style` is
//! `--diglot-stylesheet` and `--diglot-replacements` is `--diglot-replace`.
//! The short flags are unchanged except that `-ds` and `-dr` are gone: a clap
//! short flag is one character, so those two are long-only, which their help
//! text says.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};
use usfm::diagnostics::Severity;
use usfm::pipeline::OutputFormat;

/// Parse USFM and write it out as USX, HTML, JSON, SILE or a translation
/// prompt.
#[derive(Debug, Parser)]
#[command(name = "usfm", version, about)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Parse USFM files and write them out in another format.
    Parse(ParseArgs),
}

#[derive(Debug, Args)]
pub struct ParseArgs {
    /// The USFM files to read. Several are concatenated in the order given.
    #[arg(required = true, value_name = "FILES")]
    pub files: Vec<PathBuf>,

    /// The output format.
    #[arg(short, long, value_enum, default_value_t = Format::Usx)]
    pub format: Format,

    /// A Paratext stylesheet to parse against, in place of the built-in one.
    #[arg(short = 's', long, value_name = "FILE")]
    pub stylesheet: Option<PathBuf>,

    /// Write to this file instead of standard output.
    #[arg(short, long, value_name = "FILE")]
    pub output: Option<PathBuf>,

    /// A file of text replacement rules to apply to vernacular text; may be
    /// given more than once.
    #[arg(short = 'r', long = "replace", value_name = "FILE")]
    pub replace: Vec<PathBuf>,

    /// The other translation of a diglot; may be given more than once. Only
    /// `--format html` and `--format prompt` can write a pair.
    #[arg(short, long, value_name = "FILE")]
    pub diglot: Vec<PathBuf>,

    /// The stylesheet for the --diglot files (long only: -ds is not a flag).
    #[arg(long, value_name = "FILE")]
    pub diglot_stylesheet: Option<PathBuf>,

    /// Replacement rules for the --diglot files; may be given more than once
    /// (long only: -dr is not a flag).
    #[arg(long = "diglot-replace", value_name = "FILE")]
    pub diglot_replace: Vec<PathBuf>,

    /// Rewrite the output whenever one of the files above changes.
    #[arg(short, long)]
    pub watch: bool,

    /// Write no output, and exit 1, if any diagnostic is an error.
    #[arg(long)]
    pub strict: bool,

    /// Write no output, and exit 1, if any diagnostic is a warning or worse.
    #[arg(long)]
    pub deny_warnings: bool,

    /// How diagnostics are written to standard error.
    #[arg(long, value_enum, default_value_t = DiagnosticFormat::Text)]
    pub diagnostics: DiagnosticFormat,
}

impl ParseArgs {
    /// The severity at or above which a diagnostic stops the run, if any.
    /// `--deny-warnings` is the stricter of the two, so it wins when both are
    /// given.
    pub fn threshold(&self) -> Option<Severity> {
        if self.deny_warnings {
            Some(Severity::Warning)
        } else if self.strict {
            Some(Severity::Error)
        } else {
            None
        }
    }

    /// The flag that set [`ParseArgs::threshold`], for the message that
    /// reports it.
    pub fn threshold_flag(&self) -> &'static str {
        if self.deny_warnings {
            "--deny-warnings"
        } else {
            "--strict"
        }
    }
}

/// `--format`. The same list as [`OutputFormat`], as a clap value enum.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum Format {
    /// USX, the XML serialisation of the AST.
    Usx,
    /// HTML.
    Html,
    /// The AST as JSON, one object per node.
    Json,
    /// SILE's flavour of USX.
    Sile,
    /// The two sides of a diglot woven section by section.
    Prompt,
}

impl From<Format> for OutputFormat {
    fn from(format: Format) -> Self {
        match format {
            Format::Usx => OutputFormat::Usx,
            Format::Html => OutputFormat::Html,
            Format::Json => OutputFormat::Json,
            Format::Sile => OutputFormat::Sile,
            Format::Prompt => OutputFormat::Prompt,
        }
    }
}

/// `--diagnostics`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum DiagnosticFormat {
    /// One `file:line:col: severity[code]: message` line per diagnostic.
    Text,
    /// One JSON object per line.
    Json,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    /// clap checks the command for conflicting flags, bad defaults and the
    /// rest; a mistake in the struct above is a panic here rather than in
    /// someone's shell.
    #[test]
    fn the_command_is_well_formed() {
        Cli::command().debug_assert();
    }

    /// The short flags the hand-rolled loop had, still doing the same thing.
    #[test]
    fn the_old_short_flags_still_parse() {
        let cli = Cli::parse_from([
            "usfm", "parse", "-f", "html", "-s", "a.sty", "-o", "out.html", "-r", "r.txt", "-d",
            "b.usfm", "-w", "in.usfm",
        ]);
        let Command::Parse(args) = cli.command;
        assert_eq!(args.format, Format::Html);
        assert_eq!(args.stylesheet, Some("a.sty".into()));
        assert_eq!(args.output, Some("out.html".into()));
        assert_eq!(args.replace, vec![PathBuf::from("r.txt")]);
        assert_eq!(args.diglot, vec![PathBuf::from("b.usfm")]);
        assert!(args.watch);
        assert_eq!(args.files, vec![PathBuf::from("in.usfm")]);
    }

    /// `--deny-warnings` is stricter than `--strict`, and wins when both are
    /// given; neither flag means no threshold at all.
    #[test]
    fn the_stricter_threshold_wins() {
        let threshold = |flags: &[&str]| {
            let mut argv = vec!["usfm", "parse", "in.usfm"];
            argv.extend_from_slice(flags);
            let Command::Parse(args) = Cli::parse_from(argv).command;
            (args.threshold(), args.threshold_flag())
        };
        assert_eq!(threshold(&[]).0, None);
        assert_eq!(
            threshold(&["--strict"]),
            (Some(Severity::Error), "--strict")
        );
        assert_eq!(
            threshold(&["--deny-warnings"]),
            (Some(Severity::Warning), "--deny-warnings")
        );
        assert_eq!(
            threshold(&["--strict", "--deny-warnings"]),
            (Some(Severity::Warning), "--deny-warnings")
        );
    }

    /// Repeatable flags collect, and the diglot pair is long-only.
    #[test]
    fn the_repeatable_flags_collect() {
        let Command::Parse(args) = Cli::parse_from([
            "usfm",
            "parse",
            "in.usfm",
            "--replace",
            "one.txt",
            "--replace",
            "two.txt",
            "--diglot-replace",
            "three.txt",
            "--diglot-stylesheet",
            "b.sty",
        ])
        .command;
        assert_eq!(
            args.replace,
            vec![PathBuf::from("one.txt"), PathBuf::from("two.txt")]
        );
        assert_eq!(args.diglot_replace, vec![PathBuf::from("three.txt")]);
        assert_eq!(args.diglot_stylesheet, Some("b.sty".into()));
    }
}
