//! A worker's plan ledger as the status API shows it: allow-listed steps,
//! counts and the next step, read from its worktree without running git.

use std::collections::HashMap;
use std::os::unix::fs::MetadataExt as _;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::SystemTime;

use serde_json::{Map, Value, json};
use toolu_engine::ledger::io::ledger_path_for;
use toolu_protocol::host::Host;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;

use crate::model::Issue;

const COUNTS: [&str; 6] = ["total", "green", "red", "pending", "running", "stale"];
/// Every shown string is cut to this many characters.
const TEXT_MAX: usize = 160;
/// Steps shown beyond this are dropped; `summary` still counts them.
const STEPS_MAX: usize = 200;
/// Cached summaries kept before the entries of vanished ledgers are dropped.
const CACHE_MAX: usize = 256;

type Cache = Mutex<HashMap<PathBuf, ((SystemTime, u64), Value)>>;

/// `plan_summary` by ledger path, with the mtime and inode it was read at: the
/// worker's write renames a new file, so two writes inside one mtime tick still differ.
static CACHE: OnceLock<Cache> = OnceLock::new();

/// The worktree and host a worker's ledger lives under, from its launch record,
/// else the engine's copy of it. The record's `kind` is the host the worker was
/// launched on; a route can name another host after `--host` or a reroute.
pub(crate) fn locate<'a>(issue: &'a Issue, meta: &'a Value) -> (Option<&'a str>, Option<&'a str>) {
  let text = |key: &str| meta.get(key).and_then(Value::as_str);
  let worktree = text("worktree").or(issue.worktree.as_deref());
  let host = text("kind").or_else(|| (!issue.kind.is_empty()).then_some(issue.kind.as_str()));
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

/// [`plan`]'s summary counts and `next`, reread only when the ledger's mtime or file changes.
pub(crate) fn plan_summary(worktree: Option<&str>, host: Option<&str>) -> Value {
  let Some(file) = ledger_file(worktree, host) else {
    return Value::Null;
  };
  let Ok(stamp) = std::fs::metadata(&file).and_then(|meta| Ok((meta.modified()?, meta.ino())))
  else {
    return Value::Null;
  };
  let cache = CACHE.get_or_init(Cache::default);
  if let Some((at, value)) = cache.lock().ok().and_then(|map| map.get(&file).cloned())
    && at == stamp
  {
    return value;
  }
  let value = match load(&file) {
    Some(Ok(ledger)) => summary(&ledger),
    Some(Err(())) => unreadable(),
    None => Value::Null,
  };
  if let Ok(mut map) = cache.lock() {
    if map.len() >= CACHE_MAX {
      map.retain(|path, _| path.exists());
    }
    map.insert(file, (stamp, value.clone()));
  }
  value
}

/// The branch `git rev-parse --abbrev-ref HEAD` names in `worktree`, read from
/// `.git` (a directory, or a linked worktree's `gitdir:` file) and its `HEAD`:
/// `HEAD` when detached, `None` when there is no readable `HEAD` or the
/// repository stores refs in reftable.
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
  // A reftable repository keeps this placeholder in HEAD; its branch needs git.
  (branch != ".invalid").then(|| branch.to_owned())
}

fn ledger_file(worktree: Option<&str>, host: Option<&str>) -> Option<PathBuf> {
  let worktree = Path::new(worktree.filter(|tree| !tree.is_empty())?);
  let roots = Roots::new(state_env().clone(), Some(Host::parse(host?)?));
  Some(ledger_path_for(worktree, &head_branch(worktree)?, &roots))
}

/// The engine's only environment input to a ledger path, read once.
fn state_env() -> &'static Env {
  static ENV: OnceLock<Env> = OnceLock::new();
  ENV.get_or_init(|| {
    let name = "TOOLU_PROJECT_CONFIG_DIRNAME";
    let value = Env::process().get(name).map(str::to_owned);
    Env::from_pairs(value.map(|value| (name, value)))
  })
}

/// `None` for an absent ledger; `Err` for one that exists but is not a version 1 object.
fn load(file: &Path) -> Option<Result<Value, ()>> {
  let bytes = match std::fs::read(file) {
    Err(err) if err.kind() == std::io::ErrorKind::NotFound => return None,
    read => read.ok(),
  };
  let ledger = bytes
    .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
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
    .take(STEPS_MAX)
    .map(|step| {
      json!({
        "id": text(step, "id"),
        "title": text(step, "title"),
        "status": text(step, "status"),
        "activity": text(step, "activity"),
        "last_run": text(step, "last_run"),
      })
    })
    .collect();
  json!({
    "plan_doc": text(ledger, "plan_doc"),
    "updated_at": text(ledger, "updated_at"),
    "next": text(ledger, "next"),
    "summary": counts(ledger),
    "steps": steps,
  })
}

fn summary(ledger: &Value) -> Value {
  let mut value = counts(ledger);
  if let Some(map) = value.as_object_mut() {
    map.insert("next".to_owned(), text(ledger, "next"));
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

fn text(value: &Value, key: &str) -> Value {
  value
    .get(key)
    .and_then(Value::as_str)
    .map_or(Value::Null, |text| {
      json!(text.chars().take(TEXT_MAX).collect::<String>())
    })
}

#[cfg(test)]
#[path = "tests/status_plan_test.rs"]
mod tests;

#[cfg(test)]
#[path = "tests/status_plan_route_test.rs"]
mod route_tests;
