//! Validation of review evidence against committed Git content.
use super::git;
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::{Component, Path};
use toolu_runtime::host::roots::Roots;

fn text<'a>(v: &'a Value, key: &str) -> Result<&'a str, String> {
  v.get(key)
    .and_then(Value::as_str)
    .filter(|s| !s.trim().is_empty())
    .ok_or_else(|| format!("missing {key}"))
}

fn tuple(report: &Value) -> Result<(), String> {
  let route = report.get("route").ok_or("missing route")?;
  let worker = report.get("worker").ok_or("missing worker")?;
  if text(route, "source")? != "jev" {
    return Err("route must come from Jev".into());
  }
  text(route, "candidate_id")?;
  text(worker, "id")?;
  for key in ["host", "model", "effort"] {
    if text(route, key)? != text(worker, key)? {
      return Err(format!("worker {key} differs from route"));
    }
  }
  if !["claude", "codex", "cursor", "muse"].contains(&text(route, "host")?) {
    return Err("invalid host".into());
  }
  Ok(())
}

fn doc(roots: &Roots, root: &Path, path: &str) -> Result<(), String> {
  let p = Path::new(path);
  if p.components().any(|c| !matches!(c, Component::Normal(_))) || path.is_empty() {
    return Err(format!("invalid documentation path {path}"));
  }
  let lower = path.to_ascii_lowercase();
  if ![".md", ".mdx", ".rst", ".adoc", ".txt"]
    .iter()
    .any(|ext| lower.ends_with(ext))
  {
    return Err(format!("not a documentation file: {path}"));
  }
  git(roots, root, &["cat-file", "-e", &format!("HEAD:{path}")])?;
  let entry = git(roots, root, &["ls-tree", "HEAD", "--", path])?;
  if !entry.starts_with("100644 blob ") && !entry.starts_with("100755 blob ") {
    return Err(format!("not a committed regular file: {path}"));
  }
  Ok(())
}

fn coverage(roots: &Roots, root: &Path, identity: &Value, report: &Value) -> Result<(), String> {
  let files = identity
    .get("reviewed_files")
    .and_then(Value::as_array)
    .ok_or("missing files")?;
  let expected: BTreeSet<&str> = files.iter().filter_map(Value::as_str).collect();
  let mut seen = BTreeSet::new();
  let rows = report
    .get("coverage")
    .and_then(Value::as_array)
    .ok_or("missing coverage")?;
  for row in rows {
    let path = text(row, "path")?;
    if !expected.contains(path) || !seen.insert(path) {
      return Err(format!("unexpected or duplicate coverage: {path}"));
    }
    text(row, "note")?;
    let disposition = text(row, "disposition")?;
    if !["updated", "accurate", "not-needed"].contains(&disposition) {
      return Err("invalid disposition".into());
    }
    let docs = row
      .get("docs")
      .and_then(Value::as_array)
      .ok_or("missing docs array")?;
    if disposition != "not-needed" && docs.is_empty() {
      return Err("documentation reference required".into());
    }
    let mut updated = false;
    for item in docs {
      let name = item
        .as_str()
        .filter(|s| !s.trim().is_empty())
        .ok_or("invalid documentation reference")?;
      doc(roots, root, name)?;
      updated |= expected.contains(name);
    }
    if disposition == "updated" && !updated {
      return Err("updated documentation must be in the diff".into());
    }
  }
  if seen != expected {
    return Err("coverage does not match changed files".into());
  }
  Ok(())
}

/// Validate a complete report against the committed identity and documentation.
pub(super) fn validate(
  roots: &Roots,
  root: &Path,
  identity: &Value,
  report: &Value,
) -> Result<(), String> {
  for key in [
    "version",
    "repo",
    "branch",
    "base_branch",
    "diff_sha",
    "reviewed_files",
  ] {
    if report.get(key) != identity.get(key) {
      return Err(format!("review {key} is stale or missing"));
    }
  }
  tuple(report)?;
  let checks = report
    .get("checks")
    .and_then(Value::as_array)
    .ok_or("missing checks")?;
  if checks.is_empty()
    || checks
      .iter()
      .any(|v| v.as_str().is_none_or(|s| s.trim().is_empty()))
  {
    return Err("checks must be nonempty strings".into());
  }
  if !report
    .get("findings")
    .and_then(Value::as_array)
    .is_some_and(Vec::is_empty)
  {
    return Err("findings must be empty".into());
  }
  coverage(roots, root, identity, report)
}

#[cfg(test)]
#[path = "tests/report_test.rs"]
mod tests;
