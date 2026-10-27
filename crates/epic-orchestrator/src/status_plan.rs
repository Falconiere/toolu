//! A worker's plan ledger as the status API shows it: allow-listed steps,
//! counts and the next step, read from its worktree without running git.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::SystemTime;

use serde_json::{Map, Value, json};
use toolu_engine::ledger::io::{ledger_path_for, read_ledger};
use toolu_protocol::host::Host;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;

use crate::model::Issue;

const COUNTS: [&str; 6] = ["total", "green", "red", "pending", "running", "stale"];
const TEXT_MAX: usize = 160;

type Cache = Mutex<HashMap<PathBuf, (SystemTime, Value)>>;

/// `plan_summary` by ledger path, with the mtime it was read at.
static CACHE: OnceLock<Cache> = OnceLock::new();

/// The worktree and host a worker's ledger lives under: its launch record's
/// worktree, else the pane's, and the host it runs on now.
pub(crate) fn locate<'a>(
  issue: &'a Issue,
  meta: &'a Value,
  route: &'a Value,
) -> (Option<&'a str>, Option<&'a str>) {
  let text = |value: &'a Value, key: &str| value.get(key).and_then(Value::as_str);
  let worktree = text(meta, "worktree").or(issue.worktree.as_deref());
  let host = text(route, "host")
    .or_else(|| text(meta, "kind"))
    .or_else(|| (!issue.kind.is_empty()).then_some(issue.kind.as_str()));
  (worktree, host)
}

/// The plan: `null` without a worktree, host, branch or ledger, and
/// `{"error": "unreadable"}` for a ledger that cannot be shown.
pub(crate) fn plan(worktree: Option<&str>, host: Option<&str>) -> Value {
  ledger_file(worktree, host).map_or(Value::Null, |file| match load(&file) {
    Some(Ok(ledger)) => project(&ledger),
    Some(Err(())) => unreadable(),
    None => Value::Null,
  })
}

/// [`plan`]'s summary counts and `next`, reread only when the ledger's mtime moves.
pub(crate) fn plan_summary(worktree: Option<&str>, host: Option<&str>) -> Value {
  let Some(file) = ledger_file(worktree, host) else {
    return Value::Null;
  };
  let Ok(modified) = std::fs::metadata(&file).and_then(|meta| meta.modified()) else {
    return Value::Null;
  };
  let cache = CACHE.get_or_init(Cache::default);
  if let Some((at, value)) = cache.lock().ok().and_then(|map| map.get(&file).cloned())
    && at == modified
  {
    return value;
  }
  let value = match load(&file) {
    Some(Ok(ledger)) => summary(&ledger),
    Some(Err(())) => unreadable(),
    None => Value::Null,
  };
  if let Ok(mut map) = cache.lock() {
    map.insert(file, (modified, value.clone()));
  }
  value
}

/// The branch `git rev-parse --abbrev-ref HEAD` names in `worktree`, read from
/// `.git` (a directory, or a linked worktree's `gitdir:` file) and its `HEAD`:
/// `HEAD` when detached, `None` when there is no readable `HEAD`.
pub(crate) fn head_branch(worktree: &Path) -> Option<String> {
  let dot_git = worktree.join(".git");
  let git_dir = if dot_git.is_dir() {
    dot_git
  } else {
    let text = std::fs::read_to_string(&dot_git).ok()?;
    worktree.join(text.strip_prefix("gitdir:")?.trim())
  };
  let head = std::fs::read_to_string(git_dir.join("HEAD")).ok()?;
  let branch = head
    .trim()
    .strip_prefix("ref: refs/heads/")
    .unwrap_or("HEAD");
  Some(branch.to_owned())
}

fn ledger_file(worktree: Option<&str>, host: Option<&str>) -> Option<PathBuf> {
  let worktree = Path::new(worktree.filter(|tree| !tree.is_empty())?);
  let roots = Roots::new(Env::process(), Some(Host::parse(host?)?));
  Some(ledger_path_for(worktree, &head_branch(worktree)?, &roots))
}

/// `None` for an absent ledger; `Err` for one that exists but is not a version 1 object.
fn load(file: &Path) -> Option<Result<Value, ()>> {
  std::fs::symlink_metadata(file).ok()?;
  let ledger = read_ledger(file)
    .and_then(|read| serde_json::from_str::<Value>(&read.text).ok())
    .filter(|value| value.get("version").and_then(Value::as_u64) == Some(1));
  Some(ledger.ok_or(()))
}

fn unreadable() -> Value {
  json!({"error": "unreadable"})
}

fn project(ledger: &Value) -> Value {
  let steps: Vec<Value> = ledger
    .get("steps")
    .and_then(Value::as_array)
    .into_iter()
    .flatten()
    .filter(|step| step.is_object())
    .map(|step| {
      json!({
        "id": text(step, "id", usize::MAX),
        "title": text(step, "title", TEXT_MAX),
        "status": text(step, "status", usize::MAX),
        "activity": text(step, "activity", TEXT_MAX),
        "last_run": text(step, "last_run", usize::MAX),
      })
    })
    .collect();
  json!({
    "plan_doc": text(ledger, "plan_doc", usize::MAX),
    "updated_at": text(ledger, "updated_at", usize::MAX),
    "next": text(ledger, "next", usize::MAX),
    "summary": counts(ledger),
    "steps": steps,
  })
}

fn summary(ledger: &Value) -> Value {
  let mut value = counts(ledger);
  if let Some(map) = value.as_object_mut() {
    map.insert("next".to_owned(), text(ledger, "next", usize::MAX));
  }
  value
}

fn counts(ledger: &Value) -> Value {
  let summary = ledger.get("summary");
  let map: Map<String, Value> = COUNTS
    .iter()
    .map(|key| {
      let count = summary
        .and_then(|summary| summary.get(*key))
        .and_then(Value::as_u64)
        .unwrap_or(0);
      ((*key).to_owned(), json!(count))
    })
    .collect();
  Value::Object(map)
}

fn text(value: &Value, key: &str, max: usize) -> Value {
  value
    .get(key)
    .and_then(Value::as_str)
    .map_or(Value::Null, |text| {
      json!(text.chars().take(max).collect::<String>())
    })
}

#[cfg(test)]
#[path = "tests/status_plan_test.rs"]
mod tests;

#[cfg(test)]
#[path = "tests/status_plan_route_test.rs"]
mod route_tests;
