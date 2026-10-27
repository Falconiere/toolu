use serde_json::{Value, json};

use std::sync::mpsc;

use crate::PROTOCOL;
use crate::dispatch::dispatch;
use crate::paths::Paths;
use crate::server::{Engine, Fault};

fn open(root: &std::path::Path) -> Engine {
  Engine::open(Paths::at(root), None, Fault::None).expect("open")
}

fn round(engine: &mut Engine, request: &Value) -> (bool, Value) {
  let (tx, rx) = mpsc::channel();
  let mut waiters = Vec::new();
  let stop = dispatch(engine, request, &tx, &mut waiters, PROTOCOL);
  (stop, rx.try_recv().unwrap_or(Value::Null))
}

#[test]
fn host_pause_and_resume_apply_journal_and_reject_unknown_hosts() {
  let tmp = tempfile::tempdir().expect("temp");
  let mut engine = open(tmp.path());
  let (stop, value) = round(
    &mut engine,
    &json!({"op": "host-pause", "host": "cursor-agent"}),
  );
  assert!(!stop);
  assert_eq!(value["ok"], true);
  let state = toolu_engine::resources::store::read_resource_state(tmp.path()).expect("state");
  let paused = crate::host_pause::paused_hosts(&state);
  assert_eq!(
    paused
      .get(&crate::hosts::HostKind::Cursor)
      .map(|entry| entry.by.as_str()),
    Some("user")
  );
  let (stop, value) = round(
    &mut engine,
    &json!({"op": "host-resume", "host": "cursor", "by": "web"}),
  );
  assert!(!stop);
  assert_eq!(value["ok"], true);
  let state = toolu_engine::resources::store::read_resource_state(tmp.path()).expect("state");
  assert!(crate::host_pause::paused_hosts(&state).is_empty());
  let rows = crate::journal::tail(&engine.paths.journal_dir(), engine.now).expect("journal");
  let names: Vec<(&str, &str, &str)> = rows
    .iter()
    .map(|row| (row.name.as_str(), row.key.as_str(), row.note.as_str()))
    .collect();
  assert!(
    names.contains(&("host-paused", "cursor", "by=user")),
    "{names:?}"
  );
  assert!(
    names.contains(&("host-resumed", "cursor", "by=web")),
    "{names:?}"
  );
  let (_, value) = round(&mut engine, &json!({"op": "host-pause", "host": "nope"}));
  assert_eq!(
    value["error"],
    "unknown host nope; use one of claude, codex, cursor, muse"
  );
}

#[test]
fn resume_after_a_real_limit_line_clears_every_store() {
  let tmp = tempfile::tempdir().expect("temp");
  let epic = tmp.path().join("epic");
  std::fs::create_dir_all(epic.join("status")).expect("status");
  let mut engine = open(tmp.path());
  engine.world.now_ms = 1_800_000_000_000;
  engine.world.cooldown_min = 60;
  let mut issue = crate::model::Issue::blank("a", "one", &epic.display().to_string(), 0);
  issue.kind = "codex".to_owned();
  issue.stage = "running".to_owned();
  engine.world.issues.insert("a".into(), issue);
  // The cooldown is written by limit.rs's real path, from a limit line.
  crate::limit::on_failed(
    &mut engine,
    "failed",
    "rate-limited: usage limit reached",
    "a",
  )
  .expect("cool");
  assert!(epic.join("hosts.json").is_file());
  crate::disk::write_value(
    &engine.paths.registry(),
    &json!({"version": 1, "epics": [{"key": "one", "state_dir": epic.display().to_string()}]}),
  )
  .expect("registry");
  let (_, value) = round(&mut engine, &json!({"op": "host-pause", "host": "codex"}));
  assert_eq!(value["ok"], true);
  let (_, value) = round(&mut engine, &json!({"op": "host-resume", "host": "codex"}));
  assert_eq!(value["ok"], true);
  let snapshot =
    crate::status_resources::Snapshot::open(&engine.paths, engine.world.now_ms).expect("snapshot");
  let hosts = snapshot.hosts();
  let codex = hosts["items"]
    .as_array()
    .expect("items")
    .iter()
    .find(|host| host["host"] == "codex")
    .expect("codex");
  assert_eq!(codex["paused"], false);
  assert_eq!(codex["paused_by"], Value::Null);
  assert_eq!(codex["cooldown_until"], Value::Null);
  let capacity =
    crate::route::capacity::Capacity::load(&epic, Some(&engine.paths.root), engine.world.now_ms)
      .expect("capacity");
  assert!(capacity.paused.is_empty(), "{capacity:?}");
  assert!(capacity.cooling.is_empty(), "{capacity:?}");
}
