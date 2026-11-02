//! Committed documentation discovery and complete impact accounting.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde_json::Value;
use toolu_runtime::config::docs_sync::{DocsSyncKey, docs_sync_globs};
use toolu_runtime::config::load::load;
use toolu_runtime::host::roots::Roots;

use super::{Identity, git};
use crate::verdict::glob::matches_any;

/// Whether a path uses a supported documentation extension.
pub(super) fn is_document(path: &str) -> bool {
  let lower = path.to_ascii_lowercase();
  [".md", ".mdx", ".rst", ".adoc", ".txt"]
    .iter()
    .any(|extension| lower.ends_with(extension))
}

fn planning_record(path: &str) -> bool {
  let Some(rest) = path.strip_prefix("docs/toolu/") else {
    return false;
  };
  ["plans/", "brainstorms/", "specs/", "decisions/"]
    .iter()
    .any(|directory| rest.starts_with(directory))
}

/// List committed regular documentation, binding unchanged guides to their blobs.
pub(super) fn collect(roots: &Roots, root: &Path) -> Result<BTreeMap<String, String>, String> {
  let target = Roots::new(
    roots
      .env()
      .clone()
      .with("TOOLU_PROJECT_DIR", &root.to_string_lossy()),
    Some(roots.host()),
  );
  let config = load(&target, Some(root));
  if let Some(error) = &config.invalid {
    return Err(format!("cannot inventory documentation: {error}"));
  }
  let excludes = docs_sync_globs(&config, DocsSyncKey::SurfaceExcludes);
  let tree = git(roots, root, &["ls-tree", "-r", "-z", "HEAD"])?;
  Ok(
    tree
      .split('\0')
      .filter_map(|entry| {
        let (metadata, path) = entry.split_once('\t')?;
        let mut fields = metadata.split_whitespace();
        let regular = matches!(fields.next(), Some("100644" | "100755"));
        let blob = fields.next() == Some("blob");
        let hash = fields.next()?;
        (regular
          && blob
          && is_document(path)
          && !planning_record(path)
          && !matches_any(path, &excludes))
        .then(|| (path.to_owned(), hash.to_owned()))
      })
      .collect(),
  )
}

fn text<'a>(row: &'a Value, key: &str) -> Result<&'a str, String> {
  row
    .get(key)
    .and_then(Value::as_str)
    .filter(|value| !value.trim().is_empty())
    .ok_or_else(|| format!("documentation coverage requires {key}"))
}

fn sources(identity: &Identity, row: &Value, disposition: &str) -> Result<(), String> {
  let sources = row
    .get("sources")
    .and_then(Value::as_array)
    .ok_or("documentation coverage requires sources")?;
  if disposition != "not-needed" && sources.is_empty() {
    return Err("affected documentation requires a changed source".into());
  }
  let mut seen = BTreeSet::new();
  for source in sources {
    let path = source.as_str().ok_or("invalid documentation source")?;
    if !identity.reviewed_files.iter().any(|file| file == path) || !seen.insert(path) {
      return Err(format!(
        "unexpected or duplicate documentation source: {path}"
      ));
    }
  }
  Ok(())
}

/// Every inventoried document has impact evidence, including unchanged guides.
pub(super) fn validate(identity: &Identity, report: &Value) -> Result<(), String> {
  let rows = report
    .get("documentation_coverage")
    .and_then(Value::as_array)
    .ok_or("missing documentation_coverage; rerun the documentation review")?;
  let mut seen = BTreeSet::new();
  let mut affected = BTreeSet::new();
  for row in rows {
    let path = text(row, "path")?;
    if !identity.documentation_files.contains_key(path) || !seen.insert(path) {
      return Err(format!(
        "unexpected or duplicate documentation coverage: {path}"
      ));
    }
    text(row, "note")?;
    let disposition = text(row, "disposition")?;
    if !["updated", "accurate", "not-needed"].contains(&disposition) {
      return Err("invalid documentation disposition".into());
    }
    sources(identity, row, disposition)?;
    if disposition == "updated" && !identity.reviewed_files.iter().any(|file| file == path) {
      return Err(format!("updated documentation must be in the diff: {path}"));
    }
    if disposition != "not-needed" {
      affected.insert(path);
    }
  }
  if seen.len() != identity.documentation_files.len() {
    return Err("documentation coverage does not match the committed inventory".into());
  }
  cited_documents(report, &affected)
}

fn cited_documents(report: &Value, affected: &BTreeSet<&str>) -> Result<(), String> {
  let rows = report
    .get("coverage")
    .and_then(Value::as_array)
    .ok_or("missing coverage")?;
  for doc in rows
    .iter()
    .filter_map(|row| row.get("docs").and_then(Value::as_array))
    .flatten()
  {
    let path = doc.as_str().ok_or("invalid documentation reference")?;
    if !affected.contains(path) {
      return Err(format!(
        "cited documentation lacks affected coverage: {path}"
      ));
    }
  }
  Ok(())
}

#[cfg(test)]
#[path = "tests/inventory_test.rs"]
mod tests;
