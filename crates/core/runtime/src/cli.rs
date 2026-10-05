//! What a `toolu` verb receives and returns (#442): the global flags as `Ctx`,
//! and an `Outcome` that `crates/cli` prints, because plugin crates never touch
//! the standard streams (rule 14).

use std::path::PathBuf;

use toolu_protocol::exit::Exit;
use toolu_protocol::host::Host;

/// The global flags every verb sees.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Ctx {
  /// `--json`: stdout carries exactly one JSON document.
  pub json: bool,
  /// `--quiet`: drop the diagnostics of a successful run.
  pub quiet: bool,
  /// `--host`: the host named on the command line; detection (#414) decides otherwise.
  pub host: Option<Host>,
  /// `--config-dir`: where the toolu config lives instead of the host's default.
  pub config_dir: Option<PathBuf>,
}

/// What a verb produced: the exit, the data for stdout and the diagnostic for stderr.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
  /// How the verb ended.
  pub exit: Exit,
  /// Data: text, or one JSON document under `--json`.
  pub stdout: Option<String>,
  /// A diagnostic for a human.
  pub stderr: Option<String>,
}

impl Outcome {
  /// Success with `text` as the data.
  pub fn data(text: String) -> Self {
    Outcome {
      exit: Exit::Success,
      stdout: Some(text),
      stderr: None,
    }
  }

  /// `exit` with `message` as the diagnostic and no data.
  pub fn failed(exit: Exit, message: String) -> Self {
    Outcome {
      exit,
      stdout: None,
      stderr: Some(message),
    }
  }
}

#[cfg(test)]
#[path = "tests/cli_test.rs"]
mod tests;
