use serde_json::json;
use toolu_state::time::{epoch_millis, parse_iso};

use super::Snapshot;
use crate::disk::write_value;
use crate::paths::Paths;

#[test]
fn status_api_resources_project_real_lease_and_cooldown() {
  let temp = tempfile::tempdir().expect("resource root");
  let paths = Paths::at(temp.path());
  let now = epoch_millis(parse_iso("2026-10-11T12:00:00Z").expect("time"));
  write_value(
    &paths.root.join("policy.json"),
    &json!({
      "maxAgents": 3, "maxJobs": 1, "hosts": {"codex": 2}
    }),
  )
  .expect("policy");
  write_value(
    &paths.root.join("state.json"),
    &json!({
      "version": 1,
      "leases": [{
        "token": "PRIVATE-LEASE-TOKEN", "type": "agent", "key": "repo-449",
        "stateDir": "/tmp/epic", "ownerPid": 42, "stage": "running",
        "createdAt": "2026-10-11T11:59:00Z", "heartbeatAt": "2026-10-11T11:59:00Z",
        "host": "codex", "worktree": "/tmp/worktree"
      }],
      "cooldowns": {"claude": {"until": now + 60_000, "reason": "usage limit"}},
      "pressure": {"held": false, "badSince": null, "goodSince": null,
        "reason": null, "sample": {"at": now, "cpus": 4, "load": 1.0,
        "availableBytes": 800, "totalBytes": 1000, "steal": null}}
    }),
  )
  .expect("state");
  let snapshot = Snapshot::open(&paths, now).expect("resources");
  let hosts = snapshot.hosts();
  let codex = hosts["items"]
    .as_array()
    .expect("hosts")
    .iter()
    .find(|host| host["host"] == "codex")
    .expect("codex");
  let claude = hosts["items"]
    .as_array()
    .expect("hosts")
    .iter()
    .find(|host| host["host"] == "claude")
    .expect("claude");
  assert_eq!(codex["cap"], 2);
  assert_eq!(codex["running"], 1);
  assert_eq!(claude["reason"], "usage limit");
  assert!(claude["cooldown_until"].as_str().is_some());
  let machine = snapshot.machine();
  assert_eq!(machine["load"], 1.0);
  assert_eq!(machine["policy"]["max_jobs"], 1);
  assert!(!machine.to_string().contains("PRIVATE-LEASE-TOKEN"));
  assert_eq!(snapshot.jobs()["running"], json!([]));
}

#[test]
fn every_worker_host_has_a_capacity_row_with_its_policy_cap() {
  let temp = tempfile::tempdir().expect("resource root");
  let paths = Paths::at(temp.path());
  write_value(
    &paths.root.join("policy.json"),
    &json!({"maxAgents": 3, "hosts": {"muse": 1}}),
  )
  .expect("policy");
  let now = epoch_millis(parse_iso("2026-10-23T12:00:00Z").expect("time"));
  let hosts = Snapshot::open(&paths, now).expect("resources").hosts();
  let rows: Vec<(&str, u64)> = hosts["items"]
    .as_array()
    .expect("hosts")
    .iter()
    .filter_map(|host| Some((host["host"].as_str()?, host["cap"].as_u64()?)))
    .collect();
  assert_eq!(
    rows,
    [("claude", 3), ("codex", 3), ("cursor", 3), ("muse", 1)]
  );
}
