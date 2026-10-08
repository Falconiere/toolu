//! The diff a `changes` job classifies (#458).

use std::path::Path;
use std::process::Command;

use serde_json::Value;

use crate::ci_model::{ChangedFile, CiPaths};

/// A range to diff, or why every group runs instead.
pub(crate) enum DiffRange {
  /// `git diff` spec, `base...head` or `before..after`.
  Range(String),
  /// Every group is on, for this reason.
  All(String),
}

/// The range `event_name` asks for.
pub(crate) fn resolve_range(event_name: &str, event: Option<&Value>) -> DiffRange {
  match event_name {
    "pull_request" => pull_request(event),
    "push" => push(event),
    _ => DiffRange::All(format!("the {} event runs everything", shown(event_name))),
  }
}

/// The files `range` changes. Diff lines are read only for release-only paths.
pub(crate) fn read_diff(
  root: &Path,
  spec: &str,
  config: &CiPaths,
) -> Result<Vec<ChangedFile>, String> {
  let names = git(root, &["diff", "--name-only", "--no-renames", "-z", spec])?;
  let mut files = Vec::new();
  for path in names.split('\0').filter(|path| !path.is_empty()) {
    let lines = if config.matches_release(path) {
      changed_lines(root, spec, path)?
    } else {
      Vec::new()
    };
    files.push(ChangedFile {
      path: path.to_owned(),
      lines,
    });
  }
  Ok(files)
}

fn shown(event_name: &str) -> &str {
  if event_name.is_empty() {
    "unknown"
  } else {
    event_name
  }
}

fn pull_request(event: Option<&Value>) -> DiffRange {
  let Some(pull) = event.and_then(|value| value.get("pull_request")) else {
    return DiffRange::All("the pull_request event lacks base/head SHAs".to_owned());
  };
  match (sha(pull, "base"), sha(pull, "head")) {
    (Some(base), Some(head)) => DiffRange::Range(format!("{base}...{head}")),
    _ => DiffRange::All("the pull_request event lacks base/head SHAs".to_owned()),
  }
}

fn push(event: Option<&Value>) -> DiffRange {
  let Some(value) = event else {
    return DiffRange::All("the push event lacks before/after SHAs".to_owned());
  };
  let before = value.get("before").and_then(Value::as_str);
  let after = value.get("after").and_then(Value::as_str);
  match (before, after) {
    (Some(before), Some(after)) if is_sha(before) && is_sha(after) => push_range(before, after),
    _ => DiffRange::All("the push event lacks before/after SHAs".to_owned()),
  }
}

fn push_range(before: &str, after: &str) -> DiffRange {
  if before.chars().all(|ch| ch == '0') {
    DiffRange::All("the push has no previous commit".to_owned())
  } else {
    DiffRange::Range(format!("{before}..{after}"))
  }
}

fn sha<'a>(pull: &'a Value, side: &str) -> Option<&'a str> {
  pull
    .get(side)
    .and_then(|side| side.get("sha"))
    .and_then(Value::as_str)
    .filter(|text| is_sha(text))
}

fn is_sha(text: &str) -> bool {
  text.len() == 40 && text.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn changed_lines(root: &Path, spec: &str, path: &str) -> Result<Vec<String>, String> {
  let diff_text = git(
    root,
    &[
      "diff",
      "-U0",
      "--no-renames",
      "--no-color",
      spec,
      "--",
      path,
    ],
  )?;
  Ok(
    diff_text
      .lines()
      .filter(|line| line_change(line))
      .map(ToOwned::to_owned)
      .collect(),
  )
}

fn line_change(line: &str) -> bool {
  (line.starts_with('+') || line.starts_with('-'))
    && !line.starts_with("+++ ")
    && !line.starts_with("--- ")
}

fn git(root: &Path, args: &[&str]) -> Result<String, String> {
  let output = Command::new("git")
    .arg("-C")
    .arg(root)
    .args(args)
    .output()
    .map_err(|err| format!("git {}: {err}", args.join(" ")))?;
  if output.status.success() {
    return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
  }
  let stderr = String::from_utf8_lossy(&output.stderr);
  let code = output
    .status
    .code()
    .map_or_else(|| "signal".to_owned(), |code| code.to_string());
  Err(format!(
    "git {} exited {code}: {}",
    args.join(" "),
    stderr.trim()
  ))
}

#[cfg(test)]
#[path = "tests/ci_diff_test.rs"]
mod tests;
