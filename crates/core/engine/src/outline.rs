//! File outlines from one structural scan: each item's name, one-line
//! signature, first doc sentence, line range, nesting depth and visibility.
//!
//! One `ast-grep scan --inline-rules` process covers a batch of files with one
//! rule document per parser (`rules`); `parse` reads its JSON and `text`
//! derives each item's fields from the file text.

mod cache;
mod doc;
mod parse;
mod rules;
mod text;

use std::collections::{HashMap, HashSet};
use std::path::Path;

use serde::{Deserialize, Serialize};
use toolu_runtime::env::Env;
use toolu_state::content_id::blob_ids;

use crate::quality::{OUTPUT_LIMIT, run_scan};

/// One item of a file: a function, type, impl, class, method or top-level binding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
/// A file whose matches alone pass the scan's 8 MiB output cap (a minified
/// bundle) is left out of the result.
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
  let cache_dir = cache::root(env, cwd);
  let ids = cache_dir.as_ref().and_then(|_| blob_ids(env, cwd, paths));
  let (mut cached, missing) = cache_candidates(sized, cache_dir.as_deref(), ids.as_deref());
  let mut scan = Scan {
    env,
    cwd,
    rules: rules::rules(),
    found: HashMap::new(),
    too_large: HashSet::new(),
  };
  for batch in batches(missing, BATCH_BYTES) {
    scan.batch(batch)?;
  }
  let mut outlines = Vec::with_capacity(paths.len());
  let mut pending = Vec::new();
  for (index, path) in paths
    .iter()
    .enumerate()
    .filter(|(_, path)| !scan.too_large.contains(*path))
  {
    if let Some(hit) = cached.remove(&index) {
      outlines.push(hit);
      continue;
    }
    pending.push((index, outlines.len()));
    outlines.push(scanned_file(&scan, path)?);
  }
  publish_cache(
    &scan,
    paths,
    cache_dir.as_deref().zip(ids.as_deref()),
    &outlines,
    &pending,
  );
  Ok(outlines)
}

fn publish_cache(
  scan: &Scan<'_>,
  paths: &[String],
  cache: Option<(&Path, &[String])>,
  outlines: &[FileOutline],
  pending: &[(usize, usize)],
) {
  let Some((root, before)) = cache.filter(|_| !pending.is_empty()) else {
    return;
  };
  // shortcut: an edit reverted before rehash can evade validation, snapshot inputs if concurrent edits become frequent.
  let Some(after) = blob_ids(scan.env, scan.cwd, paths) else {
    return;
  };
  for &(index, position) in pending {
    if let (Some(old), Some(new), Some(file)) =
      (before.get(index), after.get(index), outlines.get(position))
      && old == new
    {
      cache::write(root, old, file);
    }
  }
}

fn cache_candidates(
  sized: Vec<(String, u64)>,
  root: Option<&Path>,
  ids: Option<&[String]>,
) -> (HashMap<usize, FileOutline>, Vec<(String, u64)>) {
  let mut cached = HashMap::new();
  let mut missing = Vec::new();
  for (index, (path, bytes)) in sized.into_iter().enumerate() {
    let hit = root
      .zip(ids.and_then(|ids| ids.get(index)))
      .and_then(|(root, id)| cache::read(root, &path, id));
    if let Some(hit) = hit {
      cached.insert(index, hit);
    } else {
      missing.push((path, bytes));
    }
  }
  (cached, missing)
}

fn scanned_file(scan: &Scan<'_>, path: &str) -> Result<FileOutline, String> {
  let bytes = std::fs::read(scan.cwd.join(path)).map_err(|err| format!("{path}: {err}"))?;
  let source = String::from_utf8_lossy(&bytes);
  // ast-grep prints `./a.rs` as `a.rs`.
  let matches = scan
    .found
    .get(path)
    .or_else(|| scan.found.get(path.trim_start_matches("./")))
    .map_or(&[][..], Vec::as_slice);
  Ok(file_outline(path, &source, matches))
}

/// One `outline` call's scans and their matches by file.
struct Scan<'a> {
  env: &'a Env,
  cwd: &'a Path,
  rules: String,
  found: HashMap<String, Vec<parse::Match>>,
  /// Files whose matches alone pass the output cap, such as minified bundles.
  too_large: HashSet<String>,
}

impl Scan<'_> {
  /// Scan `batch`; when it passes the output cap, scan its files one by one.
  fn batch(&mut self, batch: Vec<String>) -> Result<(), String> {
    match run_scan(self.env, self.cwd, &self.rules, &batch) {
      None => Err(MISSING.to_owned()),
      Some(Ok(stdout)) => {
        for one in parse::matches(&stdout)? {
          self.found.entry(one.file.clone()).or_default().push(one);
        }
        Ok(())
      }
      Some(Err(failure)) if failure.stderr_first == OUTPUT_LIMIT => {
        if batch.len() == 1 {
          self.too_large.extend(batch);
          return Ok(());
        }
        batch
          .into_iter()
          .try_for_each(|path| self.batch(vec![path]))
      }
      Some(Err(failure)) => Err(format!("ast-grep failed: {}", failure.stderr_first)),
    }
  }
}

fn file_outline(path: &str, source: &str, matches: &[parse::Match]) -> FileOutline {
  let lines: Vec<&str> = source.lines().collect();
  let language = rules::language(path).unwrap_or(rules::Language::Rust);
  let mut found: Vec<((usize, usize), Item)> = matches
    .iter()
    .map(|one| (one.bytes, text::item(&lines, one, language)))
    .collect();
  found.sort_by(|(a, _), (b, _)| a.0.cmp(&b.0).then(b.1.cmp(&a.1)));
  found.dedup_by(|(a, _), (b, _)| a == b);
  // Byte ranges, not lines, so items sharing a line stay siblings.
  let mut open: Vec<usize> = Vec::new();
  let items = found
    .into_iter()
    .map(|((_, end), mut item)| {
      open.retain(|close| *close >= end);
      item.depth = open.len();
      open.push(end);
      item
    })
    .collect();
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
