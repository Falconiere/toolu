//! Which repositories a sweep covers, and the issue records that name them:
//! `<state_dir>/issues/<key>.json`, written by `toolu epic launch` and `finish`.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde_json::Value;
use toolu_runtime::env::Env;

/// What a sweep reads from one issue record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IssueRecord {
  /// The epic state directory the record lives in.
  pub state_dir: PathBuf,
  /// The issue key, which also names its `refs/epic-wip/<key>` snapshot.
  pub key: String,
  /// The worker's stage: `running`, `merged`, `abandoned`, `cleanup-incomplete`, ...
  pub stage: String,
  /// The worker's branch.
  pub branch: Option<String>,
  /// The primary checkout the worktree was made from.
  pub checkout: Option<PathBuf>,
  /// The worker's worktree.
  pub worktree: Option<PathBuf>,
  /// Files the teardown snapshot could not commit; a non-empty list keeps the snapshot.
  pub leftover_files: usize,
}

impl IssueRecord {
  /// Whether the issue is over, so its snapshot ref has served.
  pub(crate) fn terminal(&self) -> bool {
    matches!(self.stage.as_str(), "merged" | "abandoned")
  }
}

/// Every readable issue record under `state_dirs`, in path order.
pub(crate) fn records(state_dirs: &[PathBuf]) -> Vec<IssueRecord> {
  let mut found = Vec::new();
  for state_dir in state_dirs {
    let Ok(entries) = std::fs::read_dir(state_dir.join("issues")) else {
      continue;
    };
    let mut paths: Vec<PathBuf> = entries.flatten().map(|entry| entry.path()).collect();
    paths.sort();
    for path in paths {
      if path.extension().is_some_and(|ext| ext == "json")
        && let Some(record) = read(state_dir, &path)
      {
        found.push(record);
      }
    }
  }
  found
}

fn read(state_dir: &Path, path: &Path) -> Option<IssueRecord> {
  let text = std::fs::read_to_string(path).ok()?;
  let value: Value = serde_json::from_str(&text).ok()?;
  let field = |name: &str| value.get(name).and_then(Value::as_str).map(str::to_owned);
  let absolute = |name: &str| field(name).map(PathBuf::from).filter(|p| p.is_absolute());
  Some(IssueRecord {
    state_dir: state_dir.to_path_buf(),
    key: field("key")?,
    stage: field("stage").unwrap_or_default(),
    branch: field("branch"),
    checkout: absolute("checkout"),
    worktree: absolute("worktree"),
    leftover_files: value
      .get("leftover_files")
      .and_then(Value::as_array)
      .map_or(0, Vec::len),
  })
}

/// The distinct existing repositories among `extra` and the records' checkouts,
/// each as its main worktree (a linked worktree names its repository).
pub(crate) fn discover(env: &Env, extra: &[PathBuf], records: &[IssueRecord]) -> Vec<PathBuf> {
  let mut roots = BTreeSet::new();
  let candidates = extra
    .iter()
    .cloned()
    .chain(records.iter().filter_map(|record| record.checkout.clone()));
  for candidate in candidates {
    if let Ok(real) = std::fs::canonicalize(&candidate)
      && real.join(".git").exists()
      && let Some(main) = main_worktree(env, &real)
    {
      roots.insert(main);
    }
  }
  roots.into_iter().collect()
}

/// The main worktree of the repository `dir` belongs to: the parent of its
/// common git directory.
fn main_worktree(env: &Env, dir: &Path) -> Option<PathBuf> {
  let common = super::git(
    env,
    dir,
    &["rev-parse", "--path-format=absolute", "--git-common-dir"],
  )
  .ok()?;
  let parent = Path::new(common.trim()).parent()?;
  std::fs::canonicalize(parent).ok()
}

/// The epic state directories under `parents`: children with an `issues` directory.
pub fn state_dirs_under(parents: &[PathBuf]) -> Vec<PathBuf> {
  let mut found = BTreeSet::new();
  for parent in parents {
    let Ok(entries) = std::fs::read_dir(parent) else {
      continue;
    };
    for path in entries.flatten().map(|entry| entry.path()) {
      if path.join("issues").is_dir() {
        found.insert(path);
      }
    }
  }
  found.into_iter().collect()
}

#[cfg(test)]
#[path = "tests/repos_test.rs"]
mod tests;
