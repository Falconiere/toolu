//! How a fixer ended: its report, a provider limit in its output, the Claude
//! trust prompt it may stop at, and the host errors in an OpenCode log.

use std::io::{Read as _, Seek as _, SeekFrom};
use std::path::Path;

use serde_json::{Value, json};

use crate::error::{Failure, Result};
use crate::pattern::{is, pattern};
use crate::state::write_json;

pattern!(
  HOST_LIMIT,
  r"(?i)usage limit|rate[- ]limit(ed)?\b|hit your (usage )?limit|quota (exceeded|reached)|too many requests|\b429\b|limit will reset|try again (at|in) "
);
pattern!(
  TRUST_EXTRAS,
  r"(?i)only proceed if you trust|trust this configuration|without these permissions|pre-approves|this folder adds|headershelper|mcp server|\bhooks?\b"
);
pattern!(ROUTINE_LOG, r"\blevel=(DEBUG|INFO)\b");

/// At most this much of a log is read: `--format json` logs carry every tool output.
const TAIL_BYTES: u64 = 64 * 1024;

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

/// The last `lines` lines of the log, read from its last 64 KiB without a
/// cut-off first line; empty when there is no log.
pub(crate) fn log_tail(log: &Path, lines: usize) -> String {
  let Ok(mut file) = std::fs::File::open(log) else {
    return String::new();
  };
  let size = file.metadata().map_or(0, |meta| meta.len());
  let length = size.min(TAIL_BYTES);
  let mut bytes = Vec::new();
  let read = file
    .seek(SeekFrom::Start(size - length))
    .and_then(|_| file.take(length).read_to_end(&mut bytes));
  if read.is_err() {
    return String::new();
  }
  let text = String::from_utf8_lossy(&bytes);
  let all: Vec<&str> = text.split('\n').skip(usize::from(length < size)).collect();
  all
    .get(all.len().saturating_sub(lines)..)
    .unwrap_or_default()
    .join("\n")
}

/// The host's own errors in a log tail: `error` events, and lines that are not
/// JSON nor the routine DEBUG/INFO log. Tool output and model text are left
/// out, so a fixer that wrote about rate limits is not taken for one.
pub(crate) fn host_errors(tail: &str) -> String {
  let errors = tail.split('\n').filter(|line| {
    if line.trim().is_empty() {
      return false;
    }
    match serde_json::from_str::<Value>(line) {
      Ok(value) => value.get("type").and_then(Value::as_str) == Some("error"),
      Err(_) => !is(&ROUTINE_LOG, line),
    }
  });
  errors.collect::<Vec<_>>().join("\n")
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
