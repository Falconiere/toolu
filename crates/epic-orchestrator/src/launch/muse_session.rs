//! Capture a Muse Code session for a worktree. Muse writes each session to
//! `<data>/sessions/<yyyy>/<mm>/<dd>/<uuid>/session.jsonl` at start; its
//! `runtime.session.metadata` record names the session (`stream.id`) and the
//! workspace (`payload.record.workspace_root`).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde_json::Value;
use toolu_runtime::env::Env;

use super::session::{MAX_FILES, day_dirs, fresh_head, one, records, valid_id, vanished};

/// Muse's data root: `$XDG_DATA_HOME/muse`, else `~/.local/share/muse`.
pub(crate) fn data_root(env: &Env) -> PathBuf {
  env
    .get("XDG_DATA_HOME")
    .filter(|dir| !dir.trim().is_empty())
    .map_or_else(|| env.home().join(".local").join("share"), PathBuf::from)
    .join("muse")
}

/// The session `entry` names for `worktree`, when it is the metadata record.
fn metadata_session(entry: &Value, worktree: &str) -> Option<String> {
  if entry.get("payload_type").and_then(Value::as_str) != Some("runtime.session.metadata") {
    return None;
  }
  let root = entry
    .pointer("/payload/record/workspace_root")
    .and_then(Value::as_str)?;
  let id = entry.pointer("/stream/id").and_then(Value::as_str)?;
  (root == worktree).then(|| id.to_owned())
}

/// The session directories of the days around `since` (the day folders may be
/// local time); `None` past [`MAX_FILES`].
fn session_dirs(root: &Path, since: u64) -> Result<Option<Vec<PathBuf>>, String> {
  let mut dirs = Vec::new();
  for day in day_dirs(root, &[since]) {
    let entries = match std::fs::read_dir(&day) {
      Ok(entries) => entries,
      Err(err) if vanished(&err) => continue,
      Err(err) => return Err(format!("{}: {err}", day.display())),
    };
    let named = entries
      .filter_map(Result::ok)
      .map(|entry| entry.path())
      .filter(|path| dir_name(path).is_some_and(valid_id));
    dirs.extend(named);
    if dirs.len() > MAX_FILES {
      return Ok(None);
    }
  }
  Ok(Some(dirs))
}

fn dir_name(path: &Path) -> Option<&str> {
  path.file_name().and_then(|name| name.to_str())
}

/// The one Muse session written for `worktree` since `since` (epoch ms) whose
/// metadata names its own directory.
///
/// # Errors
/// A day folder or log cannot be read for a reason other than vanishing.
pub(super) fn capture(worktree: &str, since: u64, env: &Env) -> Result<Option<String>, String> {
  let Some(dirs) = session_dirs(&data_root(env), since)? else {
    return Ok(None);
  };
  let mut found = BTreeSet::new();
  for dir in dirs {
    let Some(text) = fresh_head(&dir.join("session.jsonl"), since)? else {
      continue;
    };
    let name = dir_name(&dir).unwrap_or_default();
    let ids = records(&text).filter_map(|entry| metadata_session(&entry, worktree));
    found.extend(ids.filter(|id| id == name));
  }
  Ok(one(found))
}

#[cfg(test)]
#[path = "tests/muse_session_test.rs"]
mod tests;
