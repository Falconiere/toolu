use serde_json::json;
use toolu_engine::resources::store::read_resource_state;

use super::{pause_host, paused_hosts, resume_host};
use crate::disk::{read_value, write_value};
use crate::hosts::HostKind;
use crate::paths::Paths;

fn seed(root: &std::path::Path) {
  write_value(
    &root.join("state.json"),
    &json!({
      "version": 1, "leases": [],
      "cooldowns": {"codex": {"until": 9_999_999_999_999_u64, "reason": "usage limit"}}
    }),
  )
  .expect("state");
}

#[test]
fn pause_persists_in_the_shared_store_and_survives_a_fresh_read() {
  let temp = tempfile::tempdir().expect("temp");
  let paths = Paths::at(temp.path());
  seed(temp.path());
  pause_host(&paths, HostKind::Codex, "user", 1_760_000_000_000).expect("pause");
  // A fresh read of the same home (an engine restart) still sees the pause.
  let state = read_resource_state(temp.path()).expect("state");
  let paused = paused_hosts(&state);
  let entry = paused.get(&HostKind::Codex).expect("codex paused");
  assert_eq!(entry.by, "user");
  assert_eq!(entry.at, 1_760_000_000_000);
  assert!(!paused.contains_key(&HostKind::Claude));
}

#[test]
fn resume_clears_the_pause_and_both_cooldown_stores() {
  let temp = tempfile::tempdir().expect("temp");
  let paths = Paths::at(temp.path());
  seed(temp.path());
  pause_host(&paths, HostKind::Codex, "web", 1_760_000_000_000).expect("pause");
  let epic_a = temp.path().join("epic-a");
  let epic_b = temp.path().join("epic-b");
  for dir in [&epic_a, &epic_b] {
    write_value(
      &dir.join("hosts.json"),
      &json!({"codex": {"until": "2999-01-01T00:00:00Z", "reason": "usage limit"}, "claude": {"until": "2999-01-01T00:00:00Z", "reason": "keep"}}),
    )
    .expect("hosts");
  }
  let missing = temp.path().join("epic-missing");
  resume_host(
    &paths,
    HostKind::Codex,
    &[epic_a.clone(), epic_b.clone(), missing],
  )
  .expect("resume");
  let state = read_resource_state(temp.path()).expect("state");
  assert!(paused_hosts(&state).is_empty());
  let text = state.to_text(false);
  assert!(!text.contains("codex"), "{text}");
  for dir in [&epic_a, &epic_b] {
    let hosts = read_value(&dir.join("hosts.json")).expect("hosts");
    assert_eq!(hosts.get("codex"), None, "{hosts}");
    assert!(hosts.get("claude").is_some(), "{hosts}");
  }
}

#[test]
fn resume_without_a_pause_still_clears_the_cooldown() {
  let temp = tempfile::tempdir().expect("temp");
  let paths = Paths::at(temp.path());
  seed(temp.path());
  resume_host(&paths, HostKind::Codex, &[]).expect("resume");
  let state = read_resource_state(temp.path()).expect("state");
  assert!(!state.to_text(false).contains("codex"));
}
