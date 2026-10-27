//! Paused hosts in the shared resource store: an operator can take a host
//! out of rotation machine-wide, next to the cooldowns `limit.rs` writes
//! there. Resume clears the pause and the host's cooldown in both stores
//! (the shared store and every epic's `hosts.json`), because routing reads
//! both through `Capacity::load`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde_json::Value;
use toolu_engine::resources::store::{read_resource_state, update_resources};
use toolu_runtime::json::ordered::Ordered;

use crate::disk::{read_value, write_value};
use crate::hosts::{HostKind, parse_host_kind};
use crate::paths::Paths;

/// One paused host: who paused it and when (epoch milliseconds).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Paused {
  /// `user` (CLI/socket) or `web` (HTTP action).
  pub by: String,
  /// Epoch milliseconds.
  pub at: u64,
}

/// The paused hosts a resource state records, by canonical host.
pub(crate) fn paused_hosts(state: &Ordered) -> BTreeMap<HostKind, Paused> {
  let Some(Ordered::Object(entries)) = state.get("paused_hosts") else {
    return BTreeMap::new();
  };
  entries
    .iter()
    .filter_map(|(name, entry)| {
      let kind = parse_host_kind(name)?;
      let by = match entry.get("by") {
        Some(Ordered::String(by)) => by.clone(),
        _ => String::new(),
      };
      let at = match entry.get("at") {
        Some(Ordered::Number(number)) => number.as_u64().unwrap_or(0),
        _ => 0,
      };
      Some((kind, Paused { by, at }))
    })
    .collect()
}

/// The set of hosts paused in `state`.
pub(crate) fn paused_set(state: &Ordered) -> BTreeSet<HostKind> {
  paused_hosts(state).into_keys().collect()
}

/// Whether `kind` is paused in the store under `root`.
///
/// # Errors
/// The resource state cannot be read.
pub(crate) fn is_paused(root: &Path, kind: HostKind) -> Result<bool, String> {
  Ok(paused_set(&read_resource_state(root)?).contains(&kind))
}

/// Pause `kind`, recorded as `by` at `now_ms`, in the shared store.
///
/// # Errors
/// The resource state cannot be read or written.
pub(crate) fn pause_host(
  paths: &Paths,
  kind: HostKind,
  by: &str,
  now_ms: u64,
) -> Result<(), String> {
  let at = serde_json::Number::from(i64::try_from(now_ms).unwrap_or(i64::MAX));
  update_resources(&paths.root, |state| {
    let mut paused = state
      .get("paused_hosts")
      .cloned()
      .unwrap_or_else(|| Ordered::Object(Vec::new()));
    paused.set(
      kind.name(),
      Ordered::Object(vec![
        ("by".to_owned(), Ordered::String(by.to_owned())),
        ("at".to_owned(), Ordered::Number(at)),
      ]),
    );
    state.set("paused_hosts", paused);
    Ok(())
  })
}

/// Resume `kind`: clear its pause and its cooldown in the shared store and
/// in `hosts.json` of every epic state dir given. A dir without the file is
/// skipped.
///
/// # Errors
/// The resource state or an epic's `hosts.json` cannot be written.
pub(crate) fn resume_host(
  paths: &Paths,
  kind: HostKind,
  epic_state_dirs: &[PathBuf],
) -> Result<(), String> {
  update_resources(&paths.root, |state| {
    remove_from(state, "paused_hosts", kind.name());
    remove_from(state, "cooldowns", kind.name());
    Ok(())
  })?;
  for dir in epic_state_dirs {
    clear_epic_cooldown(dir, kind)?;
  }
  Ok(())
}

fn remove_from(state: &mut Ordered, field: &str, host: &str) {
  if let Some(Ordered::Object(entries)) = field_mut(state, field) {
    entries.retain(|(name, _)| name != host);
  }
}

fn field_mut<'a>(object: &'a mut Ordered, key: &str) -> Option<&'a mut Ordered> {
  let Ordered::Object(entries) = object else {
    return None;
  };
  let (_, value) = entries.iter_mut().find(|(name, _)| name == key)?;
  Some(value)
}

fn clear_epic_cooldown(dir: &Path, kind: HostKind) -> Result<(), String> {
  let path = dir.join("hosts.json");
  if !path.is_file() {
    return Ok(());
  }
  let mut map = match read_value(&path)? {
    Value::Object(map) => map,
    Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) | Value::Array(_) => {
      return Ok(());
    }
  };
  if map.remove(kind.name()).is_some() {
    write_value(&path, &Value::Object(map))?;
  }
  Ok(())
}

/// Every epic state dir the engine knows: the registry's, plus any in
/// `extra` (the live `World`) the registry does not name.
pub(crate) fn epic_state_dirs(paths: &Paths, extra: &[String]) -> Vec<PathBuf> {
  let mut dirs: Vec<PathBuf> = Vec::new();
  let mut push = |raw: &str| {
    let dir = PathBuf::from(raw);
    if !raw.is_empty() && !dirs.contains(&dir) {
      dirs.push(dir);
    }
  };
  if let Ok(registry) = read_value(&paths.registry())
    && let Some(rows) = registry.get("epics").and_then(Value::as_array)
  {
    for row in rows {
      if let Some(dir) = row.get("state_dir").and_then(Value::as_str) {
        push(dir);
      }
    }
  }
  for dir in extra {
    push(dir);
  }
  dirs
}

#[cfg(test)]
#[path = "tests/host_pause_test.rs"]
mod tests;
