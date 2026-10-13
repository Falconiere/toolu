//! Failed-test summary for captured Bun, Cargo and similarly shaped output.

use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;
use toolu_runtime::env::Env;

use super::cap;

static LOCATION: LazyLock<Option<Regex>> =
  LazyLock::new(|| Regex::new(r"[A-Za-z0-9_./\\-]+\.[A-Za-z]+:[0-9]+(:[0-9]+)?").ok());

#[derive(Default)]
struct Items {
  values: Vec<String>,
  seen: HashSet<String>,
  overflow: usize,
}

impl Items {
  fn add(&mut self, raw: &str, limit: usize) {
    let value = raw.trim();
    if value.is_empty() || self.seen.contains(value) {
      return;
    }
    if self.values.len() >= limit {
      self.overflow += 1;
      return;
    }
    self.seen.insert(value.to_owned());
    self.values.push(value.to_owned());
  }
}

#[derive(Default)]
struct Summary {
  failures: Items,
  errors: Items,
  locations: Items,
}

fn failure(line: &str) -> Option<String> {
  if let Some((_, rest)) = line.split_once("(fail)") {
    let candidate = rest.trim();
    if let Some((name, duration)) = candidate.rsplit_once('[')
      && duration.ends_with(']')
    {
      return Some(name.trim().to_owned());
    }
    return Some(candidate.to_owned());
  }
  let trimmed = line.trim();
  if let Some(name) = trimmed
    .strip_prefix("test ")
    .and_then(|rest| rest.split_once(" ... FAILED"))
  {
    return Some(name.0.to_owned());
  }
  for marker in ["FAIL ", "✕ ", "✗ ", "× "] {
    if let Some(name) = trimmed.strip_prefix(marker) {
      return Some(name.to_owned());
    }
  }
  None
}

fn errorish(line: &str) -> bool {
  let trimmed = line.trim_start();
  [
    "Error:",
    "error:",
    "panicked at",
    "Assertion",
    "assertion",
    "AssertionError",
    "Expected:",
    "Received:",
    "thread ",
  ]
  .iter()
  .any(|part| line.contains(part))
    || trimmed.starts_with("left:")
    || trimmed.starts_with("right:")
}

fn collect(lines: &[&str], env: &Env) -> Summary {
  let mut out = Summary::default();
  let failures_cap = cap(env, "DEBUG_MAX_FAILURES", 25);
  let errors_cap = cap(env, "DEBUG_MAX_ERRORS", 25);
  let locations_cap = cap(env, "DEBUG_MAX_LOCATIONS", 25);
  for line in lines {
    if let Some(name) = failure(line) {
      out.failures.add(&name, failures_cap);
    }
    if errorish(line) {
      out.errors.add(line, errors_cap);
    }
    if let Some(re) = LOCATION.as_ref() {
      for found in re.find_iter(line) {
        out.locations.add(found.as_str(), locations_cap);
      }
    }
  }
  out
}

fn section(out: &mut Vec<String>, title: &str, items: &Items, prefix: &str) {
  let count = if items.values.is_empty() {
    String::new()
  } else {
    items.values.len().to_string()
  };
  let plus = if items.overflow > 0 { "+" } else { "" };
  out.push(format!("{title} ({count}{plus}):"));
  out.extend(items.values.iter().map(|item| format!("{prefix}{item}")));
  if items.overflow > 0 {
    out.push(format!("  ... (+{} more)", items.overflow));
  }
}

fn raw(lines: &[&str], limit: usize) -> String {
  let mut out =
    vec!["debug-testfail: no recognizable test failures — raw input (capped):".to_owned()];
  out.extend(lines.iter().take(limit).map(|line| (*line).to_owned()));
  if lines.len() > limit {
    out.push(format!("... (+{} more lines)", lines.len() - limit));
  }
  out.join("\n")
}

/// Summarize failed test names, error lines and code locations.
pub(super) fn summarize(lines: &[&str], json: bool, env: &Env) -> String {
  let summary = collect(lines, env);
  let recognized = !summary.failures.values.is_empty()
    || !summary.errors.values.is_empty()
    || !summary.locations.values.is_empty();
  if json {
    return serde_json::json!({
      "failures": summary.failures.values,
      "errors": summary.errors.values,
      "locations": summary.locations.values,
      "recognized": recognized
    })
    .to_string();
  }
  if !recognized {
    return raw(lines, cap(env, "DEBUG_MAX_LINES", 100));
  }
  let mut out = Vec::new();
  section(&mut out, "FAILED TESTS", &summary.failures, "  - ");
  if !summary.errors.values.is_empty() {
    out.push(String::new());
    section(&mut out, "ERRORS", &summary.errors, "  ");
  }
  if !summary.locations.values.is_empty() {
    out.push(String::new());
    section(&mut out, "LOCATIONS", &summary.locations, "  ");
  }
  out.join("\n")
}

#[cfg(test)]
#[path = "tests/testfail_test.rs"]
mod tests;
