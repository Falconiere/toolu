//! One plugin's result in a verb's report.

/// How a plugin's step ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Status {
  /// Newly installed or enabled.
  Installed,
  /// Already there at the offered version.
  Already,
  /// There at another version (or shadowed), left alone; not a failure.
  Skew,
  /// The host or the edit failed.
  Failed,
  /// Not attempted: a dry run, or its core dependency failed.
  Skipped,
  /// Moved to the offered version.
  Updated,
  /// Already at the offered version.
  Current,
  /// Removed or disabled (or already absent).
  Removed,
}

impl Status {
  /// The name `--json` reports for an install or update step.
  pub(crate) fn name(self) -> &'static str {
    match self {
      Status::Installed => "installed",
      Status::Already => "already",
      Status::Skew => "skew",
      Status::Failed => "failed",
      Status::Skipped => "skipped",
      Status::Updated => "updated",
      Status::Current => "current",
      Status::Removed => "removed",
    }
  }

  /// The report's one-character mark.
  pub(crate) fn mark(self) -> char {
    match self {
      Status::Installed | Status::Updated => '+',
      Status::Already | Status::Current => '=',
      Status::Skew => '!',
      Status::Failed => 'x',
      Status::Skipped | Status::Removed => '-',
    }
  }
}

/// One plugin's step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Step {
  /// The plugin it is about.
  pub(crate) name: String,
  /// How it ended.
  pub(crate) status: Status,
  /// What happened, for a human.
  pub(crate) detail: String,
  /// The host command.
  pub(crate) argv: Vec<String>,
}

impl Step {
  /// A step.
  pub(crate) fn new(
    name: &str,
    status: Status,
    detail: impl Into<String>,
    argv: Vec<String>,
  ) -> Step {
    Step {
      name: name.to_owned(),
      status,
      detail: detail.into(),
      argv,
    }
  }

  /// Whether it failed, which makes the run exit 1.
  pub(crate) fn failed(&self) -> bool {
    self.status == Status::Failed
  }
}

#[cfg(test)]
#[path = "tests/step_test.rs"]
mod tests;
