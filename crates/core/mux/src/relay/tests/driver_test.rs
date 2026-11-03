use super::{note_prompt, quote, terminal_name, tick};
use crate::relay::config::{Config, Machine};
use crate::relay::store::{Store, Submission, Task};
use serde_json::json;
use toolu_runtime::env::Env;

#[test]
fn terminal_names_are_stable_and_fit_hc_limit() {
  assert_eq!(
    terminal_name("12345678-1234-1234-1234-123456789abc"),
    "td-12345678123412341234123456"
  );
}

#[test]
fn report_paths_are_shell_quoted() {
  assert_eq!(quote("/a b/it's"), "'/a b/it'\\''s'");
}

#[test]
fn empty_registry_tick_is_empty() {
  let dir = tempfile::tempdir().unwrap();
  assert_eq!(tick(dir.path(), &Env::default()).unwrap(), Vec::new());
}

/// One running task on worker `m`, as the driver sees it after launch.
fn running_task(store: &Store) -> Task {
  let request = Submission {
    key: "k".into(),
    origin: "origin".into(),
    base: "a".repeat(40),
    brief: "do the work".into(),
    agent: "codex".into(),
    argv: vec!["codex".into()],
    machine: None,
    group: None,
  };
  store.submit(request, b"input").unwrap();
  let machine = Machine {
    name: "m".into(),
    root: "/tmp/m".into(),
    agents: vec!["codex".into()],
    capacity: 1,
    draining: false,
  };
  let config = Config {
    version: 1,
    relay: None,
    generation: None,
    recipes: std::collections::BTreeMap::default(),
    machines: [("m".to_owned(), machine)].into_iter().collect(),
  };
  let task = store.reserve("k", &config, 1).unwrap().unwrap();
  let attempt = task.attempt.as_deref().unwrap();
  store
    .launched("k", attempt, Some("m/td-k"), "launched")
    .unwrap()
}

#[test]
fn a_blocked_worker_is_named_in_the_note_and_a_stale_diagnostic_clears() {
  let dir = tempfile::tempdir().unwrap();
  let store = Store::new(dir.path());
  let task = running_task(&store);
  let attempt = task.attempt.clone().unwrap();
  let blocked = json!({"status": {"state": "blocked"}});
  let working = json!({"status": {"state": "working"}});
  note_prompt(&store, &task, &blocked, "m/td-k").unwrap();
  let task = store.load("k").unwrap();
  assert_eq!(
    task.note.as_deref(),
    Some("blocked: the agent waits on a prompt in m/td-k")
  );
  note_prompt(&store, &task, &working, "m/td-k").unwrap();
  let task = store.load("k").unwrap();
  assert_eq!(task.note.as_deref(), Some("launched"));
  let task = store
    .annotate("k", &attempt, "command exited 1: timeout")
    .unwrap();
  note_prompt(&store, &task, &working, "m/td-k").unwrap();
  assert_eq!(store.load("k").unwrap().note.as_deref(), Some("launched"));
  let task = store.annotate("k", &attempt, "recovered").unwrap();
  note_prompt(&store, &task, &working, "m/td-k").unwrap();
  assert_eq!(store.load("k").unwrap().note.as_deref(), Some("recovered"));
}
