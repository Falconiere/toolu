//! Summaries for directory targets with over 150 supported files.

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::{Value, json};

use super::{Listed, MAX_NAMES, names};

fn module_doc(cwd: &Path, files: &[Listed<'_>]) -> Option<String> {
  for name in ["lib.rs", "mod.rs", "index.ts", "__init__.py"] {
    for (path, _) in files {
      if Path::new(path).file_name().is_some_and(|part| part == name) {
        let source = std::fs::read_to_string(cwd.join(path)).ok()?;
        let line = source.lines().find(|line| !line.trim().is_empty())?.trim();
        let text = line
          .strip_prefix("//!")
          .or_else(|| line.strip_prefix("/**"))
          .or_else(|| line.strip_prefix("\"\"\""))
          .or_else(|| line.strip_prefix("'''"))?;
        let text = text
          .trim()
          .trim_end_matches("*/")
          .trim_end_matches("\"\"\"")
          .trim_end_matches("'''")
          .trim();
        return Some(
          text
            .split_once(". ")
            .map_or(text, |(first, _)| first)
            .trim_end_matches('.')
            .to_owned()
            + ".",
        );
      }
    }
  }
  None
}

fn directory_rows<'a>(files: &'a [Listed<'a>]) -> BTreeMap<String, Vec<Listed<'a>>> {
  let mut rows: BTreeMap<String, Vec<Listed<'a>>> = BTreeMap::new();
  for &(path, file) in files {
    let dir = Path::new(path)
      .parent()
      .unwrap_or(Path::new("."))
      .to_string_lossy()
      .into_owned();
    rows.entry(dir).or_default().push((path, file));
  }
  rows
}

/// Text summary with a header, directory rows, and an omission count.
pub(super) fn text(dir: &str, files: &[Listed<'_>], cwd: &Path) -> String {
  let rows = directory_rows(files);
  let mut out = vec![format!(
    "{dir}: {} files, {} directories",
    files.len(),
    rows.len()
  )];
  for (path, group) in rows.iter().take(148) {
    let lines: usize = group
      .iter()
      .filter_map(|(_, file)| file.map(|file| file.lines))
      .sum();
    let names: Vec<&str> = group
      .iter()
      .filter_map(|(_, file)| *file)
      .flat_map(|file| names(file).0)
      .take(MAX_NAMES)
      .collect();
    let doc = module_doc(cwd, group).map_or_else(String::new, |doc| format!(" // {doc}"));
    let names = if names.is_empty() {
      String::new()
    } else {
      format!(": {}", names.join(", "))
    };
    out.push(format!(
      "{path}: {} files, {lines} lines{doc}{names}",
      group.len()
    ));
  }
  if rows.len() > 148 {
    out.push(format!("... {} more directories", rows.len() - 148));
  }
  out.join("\n")
}

/// JSON summary of the same bounded directory rows.
pub(super) fn json(dir: &str, files: &[Listed<'_>], cwd: &Path) -> Value {
  let rows = directory_rows(files);
  let dirs: Vec<Value> = rows.iter().take(148).map(|(path, group)| {
    let lines: usize = group.iter().filter_map(|(_, file)| file.map(|file| file.lines)).sum();
    let names: Vec<&str> = group.iter().filter_map(|(_, file)| *file)
      .flat_map(|file| names(file).0).take(MAX_NAMES).collect();
    json!({"path": path, "files": group.len(), "lines": lines, "doc": module_doc(cwd, group), "names": names})
  }).collect();
  json!({"dir": dir, "directories": dirs, "omitted": rows.len().saturating_sub(148)})
}
