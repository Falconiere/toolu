//! What a verb decided: a report to print, a command to hand the terminal to, or
//! a failure. Backends build these; `lib.rs` carries them out.

use std::path::PathBuf;

use serde_json::Value;
use toolu_protocol::exit::Exit;
use toolu_runtime::cli::Outcome;
use toolu_runtime::process::RunError;

/// A verb's decision.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Step {
  /// Print this and exit.
  Report(Report),
  /// Replace toolu with this command.
  Launch(Launch),
}

/// A document for `--json` and a line or two for a person.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Report {
  /// How the verb ends.
  pub(crate) exit: Exit,
  /// The `--json` document.
  pub(crate) doc: Value,
  /// The human text.
  pub(crate) text: String,
}

impl Report {
  /// The report as printed: the document under `--json`, the text otherwise.
  pub(crate) fn outcome(self, json: bool) -> Outcome {
    Outcome {
      exit: self.exit,
      stdout: Some(if json {
        self.doc.to_string()
      } else {
        self.text
      }),
      stderr: None,
    }
  }
}

/// A command to run in place of toolu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Launch {
  /// The program and its arguments.
  pub(crate) argv: Vec<String>,
  /// Where it starts.
  pub(crate) cwd: PathBuf,
}

/// Why a verb stopped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Failure {
  /// The exit code.
  pub(crate) exit: Exit,
  /// What went wrong, without the `sandbox: ` prefix.
  pub(crate) message: String,
}

impl Failure {
  /// A failure ending with `exit`.
  pub(crate) fn new(exit: Exit, message: impl Into<String>) -> Failure {
    Failure {
      exit,
      message: message.into(),
    }
  }

  /// The failure as printed on stderr.
  pub(crate) fn outcome(&self) -> Outcome {
    Outcome::failed(self.exit, format!("sandbox: {}", self.message))
  }
}

/// A process error as one phrase.
pub(crate) fn describe(err: &RunError) -> String {
  match err {
    RunError::EmptyArgv => "no command was given".to_owned(),
    RunError::ZeroTimeout | RunError::TimeoutTooLong => "the deadline is unusable".to_owned(),
    RunError::Spawn(why) | RunError::Wait(why) | RunError::Stdin(why) | RunError::Callback(why) => {
      why.clone()
    }
  }
}

#[cfg(test)]
#[path = "tests/step_test.rs"]
mod tests;
