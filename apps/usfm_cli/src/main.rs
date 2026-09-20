//! `usfm`: the command line over the `usfm` facade.
//!
//! Five modules and nothing else: [`args`] is the command line as a clap
//! struct, [`driver`] reads the files and writes `parse`'s output, [`format`]
//! is the `format` subcommand, [`watch`] is `--watch`, and [`error`] is what
//! is printed before the exit code. Every transformation of a document lives
//! in `usfm_pipeline`.
//!
//! Exit codes: 0 if the output was written, 1 if it was not — including when
//! `--strict` or `--deny-warnings` refused a document the parser had to
//! repair, and when `format --check` found a file that is not formatted.

mod args;
mod driver;
mod error;
mod format;
mod watch;

use clap::Parser;

use args::{Cli, Command, ParseArgs};
use driver::Driver;
use error::Error;

fn main() {
    let cli = Cli::parse();
    if let Err(e) = run(cli) {
        e.print();
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<(), Error> {
    match cli.command {
        Command::Parse(args) => parse(args),
        Command::Format(args) => format::run(&args),
    }
}

fn parse(args: ParseArgs) -> Result<(), Error> {
    let (mut driver, errors) = Driver::new(&args);
    for error in &errors {
        error.print();
    }
    if args.watch {
        // A file that could not be read is not fatal here: watch mode is
        // running because the files are being worked on, and the next save
        // may well create the missing one.
        watch::run(&mut driver)
    } else if errors.is_empty() {
        driver.run()
    } else {
        Err(Error::Consumed)
    }
}
