//! Why a verb stopped: the exit it maps to under the CLI contract (#442) and the
//! message printed after `toolu: `. The TypeScript installer's usage (2) and
//! missing-input (3) codes are both `Usage` (64) here; a failed step is `Failure`.

use toolu_protocol::exit::Exit;

/// A verb that could not finish.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Fail {
  /// The exit the process reports.
  pub(crate) exit: Exit,
  /// What went wrong, for a human.
  pub(crate) message: String,
}

impl Fail {
  /// A wrong command line or missing input: exit 64.
  pub(crate) fn usage(message: impl Into<String>) -> Self {
    Fail {
      exit: Exit::Usage,
      message: message.into(),
    }
  }

  /// A run that failed: exit 1.
  pub(crate) fn failed(message: impl Into<String>) -> Self {
    Fail {
      exit: Exit::Failure,
      message: message.into(),
    }
  }
}

/// The result of anything that can stop a verb.
pub(crate) type Res<T> = Result<T, Fail>;

#[cfg(test)]
#[path = "tests/fail_test.rs"]
mod tests;
