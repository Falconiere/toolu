//! Retention of shared content-addressed outline entries.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use toolu_runtime::env::Env;
use toolu_runtime::invocation::current_dir;
use toolu_state::git::common_dir;

const WARN: &str = "toolu-outline-sweep";
const RETENTION: Duration = Duration::from_hours(24 * 14);
const MAX: usize = 20_000;

/// Sweep the shared Git cache at `SessionStart` without failing startup.
pub(crate) fn sweep_outlines(env: &Env) -> Vec<String> {
  let Some(dir) = current_dir().ok().and_then(|cwd| common_dir(env, &cwd)) else {
    return Vec::new();
  };
  sweep_outline_dir(&dir.join("toolu/outline"), SystemTime::now(), MAX)
}

fn entries(dir: &Path, warnings: &mut Vec<String>) -> Vec<PathBuf> {
  let mut pending = vec![dir.to_path_buf()];
  let mut found = Vec::new();
  while let Some(folder) = pending.pop() {
    let listed = match fs::read_dir(&folder) {
      Ok(listed) => listed,
      Err(err) => {
        warnings.push(format!(
          "{WARN}: could not list {}: {err}",
          folder.display()
        ));
        continue;
      }
    };
    for entry in listed {
      let Ok(entry) = entry else {
        warnings.push(format!(
          "{WARN}: could not read an entry in {}",
          folder.display()
        ));
        continue;
      };
      let path = entry.path();
      match entry.file_type() {
        Ok(kind) if kind.is_dir() => pending.push(path),
        Ok(kind) if kind.is_file() && path.extension().is_some_and(|ext| ext == "json") => {
          found.push(path);
        }
        Ok(_) => {}
        Err(err) => warnings.push(format!("{WARN}: could not stat {}: {err}", path.display())),
      }
    }
  }
  found
}

fn remove(path: &Path, warnings: &mut Vec<String>) {
  if let Err(err) = fs::remove_file(path) {
    warnings.push(format!(
      "{WARN}: could not remove {}: {err}",
      path.display()
    ));
  }
}

/// Remove entries older than 14 days, then the oldest entries over `max`.
pub(crate) fn sweep_outline_dir(dir: &Path, now: SystemTime, max: usize) -> Vec<String> {
  let mut warnings = Vec::new();
  if !dir.is_dir() {
    return warnings;
  }
  let mut survivors = Vec::new();
  for path in entries(dir, &mut warnings) {
    match fs::metadata(&path).and_then(|meta| meta.modified()) {
      Ok(mtime) if now.duration_since(mtime).is_ok_and(|age| age > RETENTION) => {
        remove(&path, &mut warnings);
      }
      Ok(mtime) => survivors.push((mtime, path)),
      Err(err) => warnings.push(format!("{WARN}: could not judge {}: {err}", path.display())),
    }
  }
  survivors.sort();
  let excess = survivors.len().saturating_sub(max);
  for (_, path) in survivors.into_iter().take(excess) {
    remove(&path, &mut warnings);
  }
  warnings
}

#[cfg(test)]
#[path = "tests/outline_test.rs"]
mod tests;
