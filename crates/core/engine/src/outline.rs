//! File outlines from one structural scan: each item's name, one-line
//! signature, first doc sentence, line range, nesting depth and visibility.
//!
//! One `ast-grep scan --inline-rules` process covers a batch of files with one
//! rule document per parser (`rules`); `parse` reads its JSON and `text`
//! derives each item's fields from the file text.

mod parse;
mod rules;
mod text;

use std::collections::HashMap;
use std::path::Path;

use toolu_runtime::env::Env;

use crate::quality::run_scan;

/// One item of a file: a function, type, impl, class, method or top-level binding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
  /// The item's name (an impl's type).
  pub name: String,
  /// Its declaration up to the body, on one line, at most 160 characters.
  pub signature: String,
  /// The first sentence of its doc comment or docstring, at most 100 characters.
  pub doc: Option<String>,
  /// First line, one-based, including doc, attribute and decorator lines.
  pub start: usize,
  /// Last line, one-based.
  pub end: usize,
  /// How many other items enclose it.
  pub depth: usize,
  /// Whether it is exported: `pub`, `export`, or a Python name without `_`.
  pub public: bool,
}

/// One file's items, ordered by start line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileOutline {
  /// The path as the caller gave it.
  pub path: String,
  /// The file's line count.
  pub lines: usize,
  /// Its items.
  pub items: Vec<Item>,
}

/// The error when ast-grep is absent from PATH.
pub const MISSING: &str = "ast-grep not found on PATH; install it with: brew install ast-grep (or cargo install ast-grep --locked)";

/// Source bytes per scan process: its JSON runs about five times the source,
/// well under the scan runner's 8 MiB output cap.
const BATCH_BYTES: u64 = 1024 * 1024;

/// Whether `path`'s extension names a language `outline` reads.
pub fn supported(path: &str) -> bool {
  rules::language(path).is_some()
}

/// Outline each file in `paths`, relative to `cwd`, in their order.
///
/// # Errors
/// An unsupported or unreadable file, ast-grep missing ([`MISSING`]), or a
/// failed scan.
pub fn outline(env: &Env, cwd: &Path, paths: &[String]) -> Result<Vec<FileOutline>, String> {
  let mut sized = Vec::with_capacity(paths.len());
  for path in paths {
    if !supported(path) {
      return Err(format!("unsupported language: {path}"));
    }
    let meta = std::fs::metadata(cwd.join(path)).map_err(|err| format!("{path}: {err}"))?;
    sized.push((path.clone(), meta.len()));
  }
  let rules = rules::rules();
  let mut found: HashMap<String, Vec<parse::Match>> = HashMap::new();
  for batch in batches(sized, BATCH_BYTES) {
    let stdout = match run_scan(env, cwd, &rules, &batch) {
      None => return Err(MISSING.to_owned()),
      Some(Err(failure)) => return Err(format!("ast-grep failed: {}", failure.stderr_first)),
      Some(Ok(stdout)) => stdout,
    };
    for one in parse::matches(&stdout)? {
      found.entry(one.file.clone()).or_default().push(one);
    }
  }
  paths
    .iter()
    .map(|path| {
      let bytes = std::fs::read(cwd.join(path)).map_err(|err| format!("{path}: {err}"))?;
      let source = String::from_utf8_lossy(&bytes);
      // ast-grep prints `./a.rs` as `a.rs`.
      let matches = found
        .get(path.as_str())
        .or_else(|| found.get(path.trim_start_matches("./")))
        .map_or(&[][..], Vec::as_slice);
      Ok(file_outline(path, &source, matches))
    })
    .collect()
}

fn file_outline(path: &str, source: &str, matches: &[parse::Match]) -> FileOutline {
  let lines: Vec<&str> = source.lines().collect();
  let language = rules::language(path).unwrap_or(rules::Language::Rust);
  let mut items: Vec<Item> = matches
    .iter()
    .map(|one| text::item(&lines, one, language))
    .collect();
  items.sort_by(|a, b| a.start.cmp(&b.start).then(b.end.cmp(&a.end)));
  items.dedup_by(|a, b| a.start == b.start && a.end == b.end && a.name == b.name);
  let mut open: Vec<usize> = Vec::new();
  for item in &mut items {
    open.retain(|end| *end >= item.end);
    item.depth = open.len();
    open.push(item.end);
  }
  FileOutline {
    path: path.to_owned(),
    lines: lines.len(),
    items,
  }
}

/// The innermost item whose range covers one-based `line`.
pub fn enclosing(file: &FileOutline, line: usize) -> Option<&Item> {
  file
    .items
    .iter()
    .filter(|item| item.start <= line && line <= item.end)
    .max_by_key(|item| item.depth)
}

/// Group `(path, bytes)` in order so each group holds at most `limit` bytes;
/// a larger file gets a group of its own.
fn batches(sized: impl IntoIterator<Item = (String, u64)>, limit: u64) -> Vec<Vec<String>> {
  let mut groups: Vec<Vec<String>> = Vec::new();
  let mut total = 0;
  for (path, bytes) in sized {
    match groups.last_mut() {
      Some(group) if total + bytes <= limit => group.push(path),
      _ => {
        groups.push(vec![path]);
        total = 0;
      }
    }
    total += bytes;
  }
  groups
}

#[cfg(test)]
#[path = "tests/outline_test.rs"]
mod tests;
