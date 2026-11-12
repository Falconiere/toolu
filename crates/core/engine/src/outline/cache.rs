//! Path-free outline entries shared by linked worktrees.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use toolu_runtime::atomic::write_atomic;
use toolu_runtime::env::Env;
use toolu_runtime::process::{Spec, run};
use toolu_state::git::common_dir;

use super::{FileOutline, Item, rules};
use crate::quality::binary;

#[derive(Serialize, Deserialize)]
struct Entry {
  lines: usize,
  items: Vec<Item>,
}

/// A versioned cache root, or no cache outside Git or without a usable scanner.
pub(super) fn root(env: &Env, cwd: &Path) -> Option<PathBuf> {
  let common = common_dir(env, cwd)?;
  let scanner = binary(env, cwd)?;
  let mut spec = Spec::new([
    scanner.to_string_lossy().into_owned(),
    "--version".to_owned(),
  ]);
  spec.cwd = Some(cwd.to_path_buf());
  spec.env = Some(env.clone());
  let out = run(&spec).ok()?;
  if out.exit_code != 0 || out.timed_out || out.truncated {
    return None;
  }
  let version: String = out
    .stdout
    .trim()
    .chars()
    .map(|c| {
      if c.is_ascii_alphanumeric() || matches!(c, '.' | '-') {
        c
      } else {
        '_'
      }
    })
    .collect();
  (!version.is_empty()).then(|| common.join("toolu/outline").join(format!("v1-{version}")))
}

fn entry_path(root: &Path, path: &str, id: &str) -> Option<PathBuf> {
  let language = match rules::language(path)? {
    rules::Language::Rust => "rust",
    rules::Language::Script => "script",
    rules::Language::Python => "python",
  };
  Some(root.join(language).join(format!("{id}.json")))
}

/// Rehydrate a path-free entry; unreadable or invalid data is a miss.
pub(super) fn read(root: &Path, path: &str, id: &str) -> Option<FileOutline> {
  let text = std::fs::read_to_string(entry_path(root, path, id)?).ok()?;
  let entry: Entry = serde_json::from_str(&text).ok()?;
  Some(FileOutline {
    path: path.to_owned(),
    lines: entry.lines,
    items: entry.items,
  })
}

/// Atomically replace an entry; cache failures never fail outlining.
pub(super) fn write(root: &Path, id: &str, file: &FileOutline) {
  let Some(path) = entry_path(root, &file.path, id) else {
    return;
  };
  let entry = Entry {
    lines: file.lines,
    items: file.items.clone(),
  };
  if let Ok(body) = serde_json::to_string(&entry) {
    let _written = write_atomic(&path, &body);
  }
}

#[cfg(test)]
#[path = "tests/cache_test.rs"]
mod tests;
