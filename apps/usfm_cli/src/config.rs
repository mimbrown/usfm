//! Which `usfm.toml` a command reads.
//!
//! `--config FILE` names it, `--no-config` says there is none, and otherwise
//! it is the nearest one to the file being read: in its directory or one
//! above (`usfm::config::Config::discover`). A file that is found and cannot
//! be used is an error, never a silent fallback to the defaults — the run
//! would then report what the project switched off.

use std::path::Path;

use clap::Args;
use usfm::config::Config;

use crate::error::Error;

/// The two flags, the same on every command that reports diagnostics.
#[derive(Debug, Args)]
pub struct ConfigArgs {
    /// The project's configuration file, in place of the nearest `usfm.toml`
    /// to each file.
    #[arg(long, value_name = "FILE", conflicts_with = "no_config")]
    pub config: Option<std::path::PathBuf>,

    /// Read no `usfm.toml`: report everything at its own severity.
    #[arg(long)]
    pub no_config: bool,
}

impl ConfigArgs {
    /// The configuration for `file`.
    pub fn for_file(&self, file: &Path) -> Result<Config, Error> {
        if self.no_config {
            return Ok(Config::default());
        }
        let found = match &self.config {
            Some(path) => Config::load(path),
            None => {
                let file = std::path::absolute(file)?;
                Config::discover(file.parent().unwrap_or(&file))
            }
        };
        found.map_err(|e| Error::Custom(e.to_string()))
    }
}
