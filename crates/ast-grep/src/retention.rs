//! Whole-file retention for completed byte-savings session ledgers.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use toolu_runtime::env::Env;

use crate::savings::{byte_savings_dir, session_id};

/// Age after which a non-active ledger is cold and reclaimable.
pub(crate) const BYTE_SAVINGS_RETENTION: Duration = Duration::from_secs(7 * 24 * 60 * 60);

/// Maximum non-active ledgers kept after cold files are removed.
pub(crate) const BYTE_SAVINGS_MAX_COMPLETED: usize = 64;

const WARN: &str = "toolu-byte-savings-sweep";

/// Reclaim completed ledgers under the config-root byte-savings directory.
///
/// Never fails hard: I/O problems become warning strings. File contents are
/// never rewritten — only whole `*.jsonl` files are unlinked.
pub fn sweep_byte_savings(env: &Env, active_session: Option<&str>) -> Vec<String> {
  sweep_dir(&byte_savings_dir(env), active_session, SystemTime::now())
}

/// Sweep `dir` with an explicit clock (tests pin `now`).
pub(crate) fn sweep_dir(dir: &Path, active_session: Option<&str>, now: SystemTime) -> Vec<String> {
  let mut warnings = Vec::new();
  let Some(ledgers) = list_ledgers(dir, &mut warnings) else {
    return warnings;
  };
  let active = active_session
    .map(str::to_owned)
    .filter(|id| !id.is_empty());
  remove_cold(&ledgers, active.as_deref(), now, &mut warnings);
  enforce_capacity(&ledgers, active.as_deref(), &mut warnings);
  warnings
}

fn list_ledgers(dir: &Path, warnings: &mut Vec<String>) -> Option<Vec<PathBuf>> {
  if !dir.is_dir() {
    return None;
  }
  let Ok(entries) = fs::read_dir(dir) else {
    warnings.push(format!("{WARN}: could not list {}", dir.display()));
    return None;
  };
  let mut ledgers = Vec::new();
  for entry in entries {
    match entry {
      Ok(entry) => {
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "jsonl") && path.is_file() {
          ledgers.push(path);
        }
      }
      Err(_) => warnings.push(format!(
        "{WARN}: could not read an entry in {}",
        dir.display()
      )),
    }
  }
  Some(ledgers)
}

fn remove_cold(
  ledgers: &[PathBuf],
  active: Option<&str>,
  now: SystemTime,
  warnings: &mut Vec<String>,
) {
  for path in ledgers {
    if is_active(path, active) {
      continue;
    }
    if is_cold(path, now, warnings) {
      remove(path, warnings);
    }
  }
}

fn enforce_capacity(ledgers: &[PathBuf], active: Option<&str>, warnings: &mut Vec<String>) {
  let mut survivors: Vec<(SystemTime, PathBuf)> = Vec::new();
  for path in ledgers {
    if !path.is_file() || is_active(path, active) {
      continue;
    }
    match modified(path, warnings) {
      Some(mtime) => survivors.push((mtime, path.clone())),
      None => {}
    }
  }
  if survivors.len() <= BYTE_SAVINGS_MAX_COMPLETED {
    return;
  }
  survivors.sort_by_key(|(mtime, _)| *mtime);
  let excess = survivors.len() - BYTE_SAVINGS_MAX_COMPLETED;
  for (_, path) in survivors.into_iter().take(excess) {
    if is_active(&path, active) {
      continue;
    }
    remove(&path, warnings);
  }
}

fn is_active(path: &Path, active: Option<&str>) -> bool {
  active.is_some_and(|id| stem_id(path) == Some(id))
}

fn is_cold(path: &Path, now: SystemTime, warnings: &mut Vec<String>) -> bool {
  match modified(path, warnings) {
    Some(mtime) => now
      .duration_since(mtime)
      .is_ok_and(|age| age > BYTE_SAVINGS_RETENTION),
    None => false,
  }
}

fn modified(path: &Path, warnings: &mut Vec<String>) -> Option<SystemTime> {
  match fs::metadata(path).and_then(|meta| meta.modified()) {
    Ok(mtime) => Some(mtime),
    Err(err) => {
      warnings.push(format!("{WARN}: could not judge {}: {err}", path.display()));
      None
    }
  }
}

fn stem_id(path: &Path) -> Option<&str> {
  path.file_stem().and_then(|stem| stem.to_str())
}

fn remove(path: &Path, warnings: &mut Vec<String>) {
  if let Err(err) = fs::remove_file(path) {
    warnings.push(format!(
      "{WARN}: could not remove {}: {err}",
      path.display()
    ));
  }
}

/// Sanitize a SessionStart payload's `session_id` the same way append does.
pub(crate) fn active_session_from_payload(payload: &str) -> Option<String> {
  let value: serde_json::Value = serde_json::from_str(payload).ok()?;
  let raw = value.get("session_id")?;
  if matches!(raw, serde_json::Value::Null) {
    return None;
  }
  if let serde_json::Value::String(text) = raw {
    if text.is_empty() {
      return None;
    }
  }
  Some(session_id(Some(raw)))
}

#[cfg(test)]
#[path = "tests/retention_test.rs"]
mod tests;
