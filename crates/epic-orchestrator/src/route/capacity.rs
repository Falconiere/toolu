//! What already holds each host before a routing pass: this epic's occupied
//! launch records, other epics' agent leases (source and reserved
//! destination) and the cooldowns, local and shared.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde_json::Value;
use toolu_runtime::json::ordered::Ordered;

use super::pool::cooling_hosts;
use crate::graph::occupies_slot;
use crate::hosts::{HostKind, parse_host_kind};

/// Hosts in use and cooling down.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Capacity {
  /// Slots taken per host.
  pub used: BTreeMap<HostKind, u64>,
  /// Hosts cooling down after a usage limit.
  pub cooling: BTreeSet<HostKind>,
  /// Hosts an operator paused in the shared store.
  pub paused: BTreeSet<HostKind>,
  /// This epic's occupied keys and their host.
  pub active: BTreeMap<String, HostKind>,
}

impl Capacity {
  /// The capacity `state_dir` sees, plus the leases and cooldowns of
  /// `resource_root` when given.
  ///
  /// # Errors
  /// The resource state cannot be read.
  pub(crate) fn load(
    state_dir: &Path,
    resource_root: Option<&Path>,
    now_ms: u64,
  ) -> Result<Self, String> {
    let active = live_hosts(state_dir);
    let mut used: BTreeMap<HostKind, u64> = BTreeMap::new();
    for kind in active.values() {
      *used.entry(*kind).or_default() += 1;
    }
    let local = crate::disk::read_value(&state_dir.join("hosts.json")).unwrap_or(Value::Null);
    let mut capacity = Self {
      used,
      cooling: cooling_hosts(&local, now_ms),
      paused: BTreeSet::new(),
      active,
    };
    if let Some(root) = resource_root {
      capacity.shared(root, &state_dir.display().to_string(), now_ms)?;
    }
    Ok(capacity)
  }

  /// Count other epics' agent leases, both the source and a reserved
  /// destination, and the shared cooldowns.
  fn shared(&mut self, root: &Path, state_dir: &str, now_ms: u64) -> Result<(), String> {
    let state = toolu_engine::resources::store::read_resource_state(root)?;
    for lease in toolu_engine::resources::store::leases(&state) {
      if text(lease, "type").as_deref() != Some("agent") {
        continue;
      }
      let hosts: BTreeSet<HostKind> = [text(lease, "host"), text(lease, "pendingHost")]
        .into_iter()
        .flatten()
        .filter_map(|host| parse_host_kind(&host))
        .collect();
      for kind in hosts
        .into_iter()
        .filter(|kind| !owns(&self.active, lease, state_dir, *kind))
      {
        *self.used.entry(kind).or_default() += 1;
      }
    }
    self.paused = crate::host_pause::paused_set(&state);
    let cooldowns: Value = state
      .get("cooldowns")
      .and_then(|cooldowns| serde_json::from_str(&cooldowns.to_text(false)).ok())
      .unwrap_or(Value::Null);
    self.cooling.extend(cooling_hosts(&cooldowns, now_ms));
    Ok(())
  }
}

/// Whether `lease` is this epic's own live agent on `kind`, already counted.
fn owns(
  active: &BTreeMap<String, HostKind>,
  lease: &Ordered,
  state_dir: &str,
  kind: HostKind,
) -> bool {
  text(lease, "stateDir").as_deref() == Some(state_dir)
    && text(lease, "key").and_then(|key| active.get(&key).copied()) == Some(kind)
}

/// Keys of occupied launch records with a known host.
fn live_hosts(state_dir: &Path) -> BTreeMap<String, HostKind> {
  let Ok(entries) = std::fs::read_dir(state_dir.join("issues")) else {
    return BTreeMap::new();
  };
  entries
    .filter_map(Result::ok)
    .filter_map(|entry| {
      let path = entry.path();
      let key = path
        .file_name()?
        .to_str()?
        .strip_suffix(".json")?
        .to_owned();
      let record = crate::disk::read_value(&path).ok()?;
      let stage = record.get("stage").and_then(Value::as_str);
      if !occupies_slot(stage) {
        return None;
      }
      let kind = record
        .get("kind")
        .and_then(Value::as_str)
        .unwrap_or("claude");
      Some((key, parse_host_kind(kind)?))
    })
    .collect()
}

fn text(lease: &Ordered, key: &str) -> Option<String> {
  match lease.get(key) {
    Some(Ordered::String(value)) => Some(value.clone()),
    _ => None,
  }
}

#[cfg(test)]
#[path = "tests/capacity_test.rs"]
mod tests;
