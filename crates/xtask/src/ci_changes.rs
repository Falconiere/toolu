//! `cargo xtask ci-changes`: classify the event diff into CI path groups (#458).

use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::ci_diff::{DiffRange, read_diff, resolve_range};
use crate::ci_model::{Classification, all_on, classify, load_repo};
use crate::options::Options;
use crate::{Verdict, output};

/// Classify the checkout and append `<group>=true|false` lines to `GITHUB_OUTPUT`.
pub(crate) fn run(options: &Options) -> Result<Verdict, String> {
  let root = changes_root(options);
  let output_path = std::env::var("GITHUB_OUTPUT")
    .map_err(|_err| "ci-changes: GITHUB_OUTPUT is not set".to_owned())?;
  let event_name = std::env::var("GITHUB_EVENT_NAME").unwrap_or_default();
  let event = read_event(&std::env::var("GITHUB_EVENT_PATH").unwrap_or_default());
  let class = decide(&root, &event_name, event.as_ref())?;
  let body = output_text(&class);
  append_output(&output_path, &body)?;
  output::say(&format!("{}\n{body}", class.reasons.join("\n")));
  Ok(Verdict::Clean)
}

/// The classification for one event. Invalid data is an error; a bad diff is every group on.
pub(crate) fn decide(
  root: &Path,
  event_name: &str,
  event: Option<&Value>,
) -> Result<Classification, String> {
  let config = load_repo(root)?;
  Ok(match resolve_range(event_name, event) {
    DiffRange::All(reason) => all_on(&config, &reason),
    DiffRange::Range(spec) => match read_diff(root, &spec, &config) {
      Ok(files) => classify(&config, &files),
      Err(err) => all_on(&config, &format!("the diff failed: {err}")),
    },
  })
}

fn changes_root(options: &Options) -> PathBuf {
  std::env::var("CI_CHANGES_ROOT").map_or_else(|_| options.root.clone(), PathBuf::from)
}

fn read_event(path: &str) -> Option<Value> {
  if path.is_empty() {
    return None;
  }
  let text = std::fs::read_to_string(path).ok()?;
  serde_json::from_str(&text).ok()
}

fn output_text(class: &Classification) -> String {
  let mut lines = Vec::new();
  for (name, on) in &class.outputs {
    lines.push(format!("{name}={}", if *on { "true" } else { "false" }));
  }
  lines.join("\n")
}

fn append_output(path: &str, body: &str) -> Result<(), String> {
  use std::io::Write;
  let mut file = std::fs::OpenOptions::new()
    .create(true)
    .append(true)
    .open(path)
    .map_err(|err| format!("ci-changes: cannot write {path}: {err}"))?;
  writeln!(file, "{body}").map_err(|err| format!("ci-changes: cannot write {path}: {err}"))
}

#[cfg(test)]
#[path = "tests/ci_changes_test.rs"]
mod tests;
