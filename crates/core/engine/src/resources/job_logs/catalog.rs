//! Discovery and retention for managed-job logs.

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::Path;
use std::time::SystemTime;

use toolu_runtime::json::ordered::Ordered;
use toolu_state::time::iso_millis;

use super::{Entry, KEEP_PER_KEY, RETAIN, dir, io, lock, lock_path, meta_path, metadata};
use crate::resources::store::{leases, read_resource_state};

fn scan(root: &Path) -> Result<Vec<Entry>, String> {
  let folder = dir(root);
  let entries = match fs::read_dir(&folder) {
    Ok(entries) => entries,
    Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
    Err(err) => return Err(io(&folder, err)),
  };
  let state = read_resource_state(root)?;
  let running: HashSet<&str> = leases(&state)
    .iter()
    .filter_map(|lease| {
      if let Some(Ordered::String(text)) = lease.get("token") {
        Some(text.as_str())
      } else {
        None
      }
    })
    .collect();
  let mut logs = Vec::new();
  for entry in entries {
    let entry = entry.map_err(|err| io(&folder, err))?;
    let path = entry.path();
    if path.extension().is_none_or(|ext| ext != "json")
      || !entry.file_type().map_err(|err| io(&path, err))?.is_file()
    {
      continue;
    }
    if let Some(log) = scan_entry(&path, &running)? {
      logs.push(log);
    }
  }
  Ok(logs)
}

fn scan_entry(path: &Path, running: &HashSet<&str>) -> Result<Option<Entry>, String> {
  let _held = lock(&lock_path(&path.with_extension("log")))?;
  Ok(scan_entry_locked(path, running))
}

fn scan_entry_locked(path: &Path, running: &HashSet<&str>) -> Option<Entry> {
  let meta = metadata(path).ok()?;
  let key = meta.get("key")?.as_str()?;
  let label = meta.get("label")?.as_str()?;
  let started_at = meta.get("started_at")?.as_str()?;
  let base = meta.get("base_offset")?.as_u64()?;
  let log = path.with_extension("log");
  let info = fs::symlink_metadata(&log).ok()?;
  if !info.file_type().is_file() {
    return None;
  }
  let name = path.file_stem()?.to_str()?;
  let updated = info.modified().unwrap_or(SystemTime::UNIX_EPOCH);
  Some(Entry {
    id: format!("job-{name}"),
    key: key.to_owned(),
    label: label.to_owned(),
    state: if running.contains(name) {
      "running"
    } else {
      "done"
    },
    size: base.saturating_add(info.len()),
    started_at: started_at.to_owned(),
    updated_at: iso_millis(updated),
    path: log,
    updated,
  })
}

fn remove(path: &Path) -> Result<(), String> {
  match fs::remove_file(path) {
    Ok(()) => Ok(()),
    Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
    Err(err) => Err(io(path, err)),
  }
}

fn prune(logs: &mut Vec<Entry>) -> Result<(), String> {
  logs.sort_by(|a, b| {
    b.started_at
      .cmp(&a.started_at)
      .then_with(|| b.id.cmp(&a.id))
  });
  let mut per_key = BTreeMap::<String, usize>::new();
  let mut kept = Vec::with_capacity(logs.len());
  for entry in logs.drain(..) {
    let old = SystemTime::now()
      .duration_since(entry.updated)
      .unwrap_or_default()
      > RETAIN;
    let rank = per_key.entry(entry.key.clone()).or_default();
    if entry.state == "running" || (!old && *rank < KEEP_PER_KEY) {
      if entry.state != "running" {
        *rank += 1;
      }
      kept.push(entry);
      continue;
    }
    let _held = lock(&lock_path(&entry.path))?;
    remove(&entry.path)?;
    remove(&meta_path(&entry.path))?;
    remove(&lock_path(&entry.path))?;
  }
  *logs = kept;
  Ok(())
}

/// Discover and prune logs for all keys, returning one key's newest first.
///
/// # Errors
/// The store, resource leases or retention files cannot be read or changed.
pub fn list(root: &Path, key: &str) -> Result<Vec<Entry>, String> {
  let mut logs = scan(root)?;
  prune(&mut logs)?;
  Ok(logs.into_iter().filter(|entry| entry.key == key).collect())
}

#[cfg(test)]
#[path = "tests/catalog_test.rs"]
mod tests;
