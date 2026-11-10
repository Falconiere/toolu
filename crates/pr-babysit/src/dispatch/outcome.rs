//! How a fixer ended: its report, a provider limit in its output, the Claude
//! trust prompt it may stop at.

use std::path::Path;

use serde_json::{Value, json};

use crate::error::{Failure, Result};
use crate::pattern::{is, pattern};
use crate::state::write_json;

// Muse says `model limit reached` and `You've used your <plan> quota`.
pattern!(
  HOST_LIMIT,
  r"(?i)usage limit|rate[- ]limit(ed)?\b|hit your (usage )?limit|quota (exceeded|reached)|too many requests|\b429\b|limit will reset|try again (at|in) |model limit reached|you've used your [a-z0-9 -]*quota"
);
pattern!(
  TRUST_EXTRAS,
  r"(?i)only proceed if you trust|trust this configuration|without these permissions|pre-approves|this folder adds|headershelper|mcp server|\bhooks?\b"
);

/// `done` or `reported_failed` from the report, else `host_limited` when the
/// output names a provider limit, else `no_report`.
pub(crate) fn fixer_outcome(report: &Path, output: &str) -> &'static str {
  let parsed = std::fs::read_to_string(report)
    .ok()
    .and_then(|text| serde_json::from_str::<Value>(&text).ok());
  match parsed
    .as_ref()
    .and_then(|report| report.get("status")?.as_str())
  {
    Some("done") => "done",
    Some("failed") => "reported_failed",
    _ if is(&HOST_LIMIT, output) => "host_limited",
    _ => "no_report",
  }
}

fn names(line: &str, path: &str) -> bool {
  let line = line.trim_end();
  line == path || line.strip_prefix(' ') == Some(path)
}

/// Whether the pane's last screen is Claude's plain folder-trust prompt for
/// `path`, with nothing (hooks, MCP servers, settings) that trusting would approve.
pub(crate) fn is_claude_trust_prompt(text: &str, path: &str) -> bool {
  let Some(marker) = text.rfind("Accessing workspace:") else {
    return false;
  };
  let screen = text.get(marker..).unwrap_or_default();
  if !screen.split('\n').any(|line| names(line, path)) {
    return false;
  }
  if !screen.contains("Yes, I trust this folder") || !screen.contains("No, exit") {
    return false;
  }
  let prose: Vec<&str> = screen
    .split('\n')
    .filter(|line| !names(line, path))
    .collect();
  let prose = prose
    .join(" ")
    .split_whitespace()
    .collect::<Vec<_>>()
    .join(" ");
  !is(&TRUST_EXTRAS, &prose)
}

/// `fixer-report <file> done|failed [--note <text>]`: the fixer's atomic report.
///
/// # Errors
/// `usage` for another status; a write failure.
pub(crate) fn report(file: &Path, status: &str, note: Option<&str>, at: &str) -> Result<Value> {
  if !matches!(status, "done" | "failed") {
    return Err(Failure::usage(
      "fixer-report: status must be done or failed",
    ));
  }
  let document = json!({"version": 1, "status": status, "note": note, "at": at});
  write_json(file, &document, "report")?;
  Ok(document)
}

#[cfg(test)]
#[path = "tests/outcome_test.rs"]
mod tests;
