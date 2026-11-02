use super::{connection, project, status};
use crate::relay::{
  config::{Config, Machine},
  store::Task,
};
use serde_json::json;

fn machine(name: &str, draining: bool) -> Machine {
  Machine {
    name: name.into(),
    root: "/work".into(),
    agents: vec!["codex".into()],
    capacity: 3,
    draining,
  }
}

fn task(key: &str, phase: &str, machine: &Machine) -> Task {
  serde_json::from_value(json!({
    "request":{"key":key,"origin":"repo","base":"a".repeat(40),"brief":"work","agent":"codex","argv":["codex"],"machine":null,"group":null},
    "phase":phase,"attempt":"attempt","machine":machine.name,"operation":"op","terminal":null,"result":null,
    "failures":0,"next_at":0,"note":null,"worker":machine
  })).unwrap()
}

#[test]
fn projects_connections_and_capacity_without_assuming_prerequisites() {
  let alpha = machine("macbook", false);
  let drain = machine("drain", true);
  let mut config = Config::default();
  config.machines.insert(alpha.name.clone(), alpha.clone());
  config.machines.insert(drain.name.clone(), drain);
  let tasks = vec![
    task("one", "held", &alpha),
    task("two", "cancelling", &alpha),
    task("done", "done", &alpha),
  ];
  let hc = vec![
    json!({"host":"192", "id":"2a56600dfafd8414bc60e820fb0bf321", "name":"macbook", "session":null, "state":"connected", "target":"macbook", "temporary":false, "version":"0.1.16", "via":false}),
  ];
  let rows = project(
    &config,
    &tasks,
    Some(&hc),
    &std::collections::BTreeMap::new(),
    0,
  );
  assert_eq!(rows[0]["connection"], "unconfigured");
  assert_eq!(rows[0]["available"], 0);
  assert_eq!(rows[1]["connection"], "connected");
  assert_eq!(rows[1]["occupied"], 2);
  assert_eq!(rows[1]["available"], 1);
  assert_eq!(connection(&alpha, None), "unknown");
  assert_eq!(
    connection(
      &alpha,
      Some(&[json!({"name":"macbook", "state":"disconnected"})])
    ),
    "disconnected"
  );
}

#[test]
fn exposes_preparation_failures_with_retry_time() {
  let worker = machine("alpha", false);
  let mut config = Config::default();
  config.machines.insert(worker.name.clone(), worker.clone());
  let mut failure = task("job", "queued", &worker);
  failure.machine = None;
  failure.failures = 1;
  failure.next_at = 42;
  failure.note = Some("missing agent".into());
  let rows = project(
    &config,
    &[failure],
    None,
    &std::collections::BTreeMap::new(),
    0,
  );
  assert_eq!(
    rows[0]["preparation_errors"],
    json!([{"key":"job","note":"missing agent","next_retry":42}])
  );
  assert_eq!(rows[0]["connection"], "unknown");
}

#[test]
fn unavailable_hc_preserves_registry() {
  let dir = tempfile::tempdir().unwrap();
  crate::relay::config::update(dir.path(), |config| {
    crate::relay::config::register(config, machine("alpha", false))
  })
  .unwrap();
  let env = toolu_runtime::env::Env::from_pairs([("PATH", "/nonexistent")]);
  let result = status(dir.path(), &env).unwrap();
  assert_eq!(result["workers"][0]["name"], "alpha");
  assert_eq!(result["workers"][0]["connection"], "unknown");
  assert!(result["connection_error"].as_str().is_some());
}

#[test]
fn readiness_exposes_preparation_cooldown_across_restart() {
  let dir = tempfile::tempdir().unwrap();
  let worker = machine("alpha", false);
  let config = crate::relay::config::update(dir.path(), |config| {
    crate::relay::config::register(config, worker)
  })
  .unwrap();
  let store = crate::relay::store::Store::new(dir.path());
  let request = serde_json::from_value(json!({"key":"job", "origin":"repo", "base":"a".repeat(40),
    "brief":"work", "agent":"codex", "argv":["codex"], "machine":null, "group":null}))
  .unwrap();
  store.submit(request, b"input").unwrap();
  let task = store.reserve("job", &config, 100).unwrap().unwrap();
  let now = u64::MAX - 10;
  store
    .retry("job", task.attempt.as_deref().unwrap(), now, "unreachable")
    .unwrap();
  let env = toolu_runtime::env::Env::from_pairs([("PATH", "/nonexistent")]);
  let result = status(dir.path(), &env).unwrap();
  assert_eq!(result["workers"][0]["preparation_cooldown_until"], now + 4);
  assert_eq!(result["workers"][0]["available"], 0);
  assert_eq!(result["workers"][0]["occupied"], 0);
}
