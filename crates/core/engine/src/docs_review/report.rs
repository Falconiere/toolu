//! Validation of review evidence against committed Git content.
use super::{Identity, git, worker_host};
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
  if worker_host(text(route, "host")?).is_none() {
    return Err("invalid host".into());
  }
  Ok(())
}

fn doc(path: &str) -> Result<(), String> {
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
  Ok(())
}

/// Every one of `docs` is a regular file committed at `HEAD`, read in one listing.
fn committed(roots: &Roots, root: &Path, docs: &BTreeSet<&str>) -> Result<(), String> {
  if docs.is_empty() {
    return Ok(());
  }
  let mut args = vec!["--literal-pathspecs", "ls-tree", "-z", "HEAD", "--"];
  args.extend(docs.iter().copied());
  let listing = git(roots, root, &args)?;
  let regular: BTreeSet<&str> = listing
    .split('\0')
    .filter_map(|entry| {
      let (meta, path) = entry.split_once('\t')?;
      (meta.starts_with("100644 blob ") || meta.starts_with("100755 blob ")).then_some(path)
    })
    .collect();
  docs
    .iter()
    .find(|doc| !regular.contains(*doc))
    .map_or(Ok(()), |doc| {
      Err(format!("not a committed regular file: {doc}"))
    })
}

fn coverage(roots: &Roots, identity: &Identity, report: &Value) -> Result<(), String> {
  let expected: BTreeSet<&str> = identity.reviewed_files.iter().map(String::as_str).collect();
  let mut seen = BTreeSet::new();
  let mut cited = BTreeSet::new();
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
      doc(name)?;
      cited.insert(name);
      updated |= expected.contains(name);
    }
    if disposition == "updated" && !updated {
      return Err("updated documentation must be in the diff".into());
    }
  }
  if seen != expected {
    return Err("coverage does not match changed files".into());
  }
  committed(roots, identity.root(), &cited)
}

/// Validate a complete report against the committed identity and documentation.
pub(super) fn validate(roots: &Roots, identity: &Identity, report: &Value) -> Result<(), String> {
  let expected = identity.json();
  for key in [
    "version",
    "repo",
    "branch",
    "base_branch",
    "diff_sha",
    "reviewed_files",
  ] {
    if report.get(key) != expected.get(key) {
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
  coverage(roots, identity, report)
}

#[cfg(test)]
#[path = "tests/report_test.rs"]
mod tests;
