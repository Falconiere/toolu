//! What a check found, and how it reads to the agent: each failed check, then a
//! corrected example it can copy.

use crate::format::Violation;

/// The command that checks a message or branch before it runs.
pub(crate) const CHECK_HINT: &str = "Check first with `toolu git-better check --message-file <file>` or `toolu git-better check --branch <name>`.";

/// What was checked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Target {
  /// A commit message.
  Message,
  /// A branch name.
  Branch,
}

impl Target {
  /// The name `--json` documents use for it.
  pub(crate) fn as_str(self) -> &'static str {
    match self {
      Target::Message => "message",
      Target::Branch => "branch",
    }
  }
}

/// One message or branch that failed its checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Finding {
  /// What was checked.
  pub(crate) target: Target,
  /// The branch name; empty for a message.
  pub(crate) name: String,
  /// Every failed check.
  pub(crate) violations: Vec<Violation>,
  /// A corrected version.
  pub(crate) example: String,
}

impl Finding {
  fn heading(&self) -> String {
    match self.target {
      Target::Message => "Commit message:".to_owned(),
      Target::Branch => format!("Branch `{}`:", self.name),
    }
  }
}

/// The findings as the agent reads them, after `headline`.
pub(crate) fn render(headline: &str, findings: &[Finding]) -> String {
  let mut lines = vec![headline.to_owned()];
  for finding in findings {
    lines.push(finding.heading());
    lines.extend(
      finding
        .violations
        .iter()
        .map(|violation| format!("- {}", violation.message)),
    );
    lines.push("Write it as:".to_owned());
    lines.extend(finding.example.lines().map(|line| {
      if line.is_empty() {
        String::new()
      } else {
        format!("  {line}")
      }
    }));
  }
  lines.push(CHECK_HINT.to_owned());
  lines.join("\n")
}

#[cfg(test)]
#[path = "tests/report_test.rs"]
mod tests;
