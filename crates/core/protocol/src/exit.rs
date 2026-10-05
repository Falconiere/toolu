//! The exit codes of every `toolu` command (#442). Hooks end with `Success` or
//! `Blocked`; the sysexits codes cover usage, missing dependencies and retries.

/// How a `toolu` command ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit {
  /// The command did what was asked.
  Success,
  /// The command ran and failed.
  Failure,
  /// A hook or gate blocked or denied the action.
  Blocked,
  /// The command line was wrong (sysexits `EX_USAGE`).
  Usage,
  /// A dependency such as `gh` is missing or unreachable (sysexits `EX_UNAVAILABLE`).
  Unavailable,
  /// A temporary failure the caller may retry (sysexits `EX_TEMPFAIL`).
  TempFail,
}

impl Exit {
  /// Every exit, in code order.
  pub const ALL: [Exit; 6] = [
    Exit::Success,
    Exit::Failure,
    Exit::Blocked,
    Exit::Usage,
    Exit::Unavailable,
    Exit::TempFail,
  ];

  /// The process exit status.
  pub fn code(self) -> u8 {
    match self {
      Exit::Success => 0,
      Exit::Failure => 1,
      Exit::Blocked => 2,
      Exit::Usage => 64,
      Exit::Unavailable => 69,
      Exit::TempFail => 75,
    }
  }

  /// The name `--json` documents use for it.
  pub fn name(self) -> &'static str {
    match self {
      Exit::Success => "success",
      Exit::Failure => "failure",
      Exit::Blocked => "blocked",
      Exit::Usage => "usage",
      Exit::Unavailable => "unavailable",
      Exit::TempFail => "tempfail",
    }
  }

  /// One line for help text and the command tree.
  pub fn meaning(self) -> &'static str {
    match self {
      Exit::Success => "the command did what was asked",
      Exit::Failure => "the command ran and failed",
      Exit::Blocked => "a hook or gate blocked or denied the action",
      Exit::Usage => "the command line is wrong",
      Exit::Unavailable => "a dependency (for example `gh`) is missing or unreachable",
      Exit::TempFail => "a temporary failure; the caller may retry",
    }
  }
}

#[cfg(test)]
#[path = "tests/exit_test.rs"]
mod tests;
