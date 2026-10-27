use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::time::SystemTime;

use serde_json::{Value, json};
use toolu_engine::resources::leases::{LeaseKind, LeaseRequest, acquire_lease};
use toolu_engine::resources::transitions::{MigrationCaps, prepare_agent_migration};
use toolu_runtime::json::ordered::Ordered;

use super::Capacity;
use crate::hosts::HostKind::{Claude, Codex};

const OCCUPIED_STAGES: [&str; 7] = [
  "starting",
  "uncertain",
  "replacing",
  "running",
  "awaiting_merge",
  "cleaning",
  "cleanup-incomplete",
];

fn record(state: &Path, key: &str, body: &Value) {
  let path = state.join("issues").join(format!("{key}.json"));
  std::fs::create_dir_all(path.parent().expect("parent")).expect("dir");
  std::fs::write(path, body.to_string()).expect("record");
}

fn agent(root: &Path, key: &str, state_dir: &str, host: &str) -> String {
  let request = LeaseRequest {
    kind: LeaseKind::Agent,
    key: key.into(),
    state_dir: state_dir.into(),
    host: Some(host.into()),
    host_cap: None,
    epic_cap: None,
    worktree: None,
  };
  let lease = acquire_lease(root, &request, SystemTime::now()).expect("lease");
  match lease.get("token") {
    Some(Ordered::String(token)) => token.clone(),
    _ => String::new(),
  }
}

#[test]
fn every_occupied_stage_takes_a_slot_and_an_unknown_host_takes_none() {
  let tmp = tempfile::tempdir().expect("temp");
  for (index, stage) in OCCUPIED_STAGES.iter().enumerate() {
    record(
      tmp.path(),
      &format!("owned-{index}"),
      &json!({"stage": stage}),
    );
  }
  for stage in ["merged", "abandoned", "unknown"] {
    record(
      tmp.path(),
      &format!("released-{stage}"),
      &json!({"stage": stage, "kind": "codex"}),
    );
  }
  record(
    tmp.path(),
    "x-1",
    &json!({"stage": "running", "kind": "gemini"}),
  );
  let capacity = Capacity::load(tmp.path(), None, 0).expect("capacity");
  let taken = u64::try_from(OCCUPIED_STAGES.len()).expect("count");
  assert_eq!(capacity.used, BTreeMap::from([(Claude, taken)]));
  assert_eq!(capacity.active.len(), OCCUPIED_STAGES.len());
}

#[test]
fn other_epics_count_their_source_and_reserved_destination() {
  let tmp = tempfile::tempdir().expect("temp");
  let root = tmp.path().join("resources");
  let state = tmp.path().join("state");
  let token = agent(&root, "other-1", "other-epic", "claude");
  let caps = MigrationCaps {
    host_cap: 1,
    epic_cap: None,
  };
  prepare_agent_migration(&root, &token, "codex", caps, SystemTime::now()).expect("reserve");
  record(
    &state,
    "mine-1",
    &json!({"stage": "running", "kind": "claude"}),
  );
  agent(&root, "mine-1", &state.display().to_string(), "claude");
  let capacity = Capacity::load(&state, Some(&root), 0).expect("capacity");
  assert_eq!(capacity.used, BTreeMap::from([(Claude, 2), (Codex, 1)]));
}

#[test]
fn local_cooldowns_cool_a_host_and_a_missing_root_counts_nothing() {
  let tmp = tempfile::tempdir().expect("temp");
  let state = tmp.path().join("state");
  crate::disk::write_value(
    &state.join("hosts.json"),
    &json!({"claude": {"until": "2999-01-01T00:00:00Z"}, "codex": {"until": 5}}),
  )
  .expect("hosts");
  let capacity = Capacity::load(&state, None, 10).expect("capacity");
  assert_eq!(capacity.cooling, BTreeSet::from([Claude]));
  let missing = Capacity::load(&tmp.path().join("none"), Some(&tmp.path().join("r")), 10);
  assert_eq!(missing.map(|capacity| capacity.used), Ok(BTreeMap::new()));
}

#[test]
fn shared_paused_hosts_are_reported_apart_from_cooling() {
  let tmp = tempfile::tempdir().expect("temp");
  let root = tmp.path().join("resources");
  std::fs::create_dir_all(&root).expect("root");
  let state = json!({
    "version": 1,
    "leases": [],
    "cooldowns": {},
    "paused_hosts": {"codex": {"by": "user", "at": 5}}
  });
  std::fs::write(root.join("state.json"), state.to_string()).expect("state");
  let capacity = Capacity::load(&tmp.path().join("state"), Some(&root), 10).expect("capacity");
  assert_eq!(capacity.paused, BTreeSet::from([Codex]));
  assert_eq!(capacity.cooling, BTreeSet::new());
}

#[test]
fn shared_cooldowns_cool_a_host_until_they_expire() {
  let tmp = tempfile::tempdir().expect("temp");
  let root = tmp.path().join("resources");
  std::fs::create_dir_all(&root).expect("root");
  let state = json!({
    "version": 1,
    "leases": [],
    "cooldowns": {
      "codex": {"until": 20, "reason": "usage limit"},
      "cursor": {"until": 5, "reason": "usage limit"}
    }
  });
  std::fs::write(root.join("state.json"), state.to_string()).expect("state");
  let capacity = Capacity::load(&tmp.path().join("state"), Some(&root), 10).expect("capacity");
  assert_eq!(capacity.cooling, BTreeSet::from([Codex]));
  std::fs::write(root.join("state.json"), "{\"version\": 2}").expect("state");
  let invalid = Capacity::load(&tmp.path().join("state"), Some(&root), 10);
  assert_eq!(
    invalid,
    Err("invalid resource state; reconcile ownership before launching".to_owned())
  );
}
