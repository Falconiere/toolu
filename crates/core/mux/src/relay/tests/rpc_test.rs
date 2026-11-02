use super::*;
use crate::relay::config::{Config, Machine};

fn invoke(root: &Path, value: &Value) -> Result<Value, String> {
  receive(
    root,
    &Env::from_pairs([("PATH", "/usr/bin")]),
    &value.to_string(),
  )
}
fn request() -> Submission {
  Submission {
    key: "task".into(),
    origin: "origin".into(),
    base: "a".repeat(40),
    brief: "work".into(),
    agent: "codex".into(),
    argv: vec!["work".into()],
    machine: None,
    group: None,
  }
}
fn setup(root: &Path) {
  let config = Config {
    relay: Some("relay".into()),
    generation: Some("first".into()),
    ..Config::default()
  };
  invoke(
    root,
    &json!({"op":"enroll", "expected":Config::default(), "config":config}),
  )
  .unwrap();
}

fn worker_config(root: &Path) -> Config {
  let mut config = Config {
    relay: Some("relay".into()),
    generation: Some("first".into()),
    ..Config::default()
  };
  config.machines.insert(
    "worker".into(),
    Machine {
      name: "worker".into(),
      root: "/tmp/worker".into(),
      agents: vec!["codex".into()],
      capacity: 1,
      draining: false,
    },
  );
  invoke(
    root,
    &json!({"op":"enroll", "expected":Config::default(), "config":config}),
  )
  .unwrap();
  config
}
#[test]
fn chunk_retries_are_immutable_and_submit_rejects_invalid_input() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path();
  setup(root);
  let chunk = json!({"op":"chunk", "key":"task", "index":0, "bytes":[1,2,3]});
  invoke(root, &chunk).unwrap();
  invoke(root, &chunk).unwrap();
  assert!(
    invoke(
      root,
      &json!({"op":"chunk", "key":"task", "index":0, "bytes":[1,2,4]})
    )
    .is_err()
  );
  assert!(
    invoke(
      root,
      &json!({"op":"chunk", "key":"../escape", "index":0, "bytes":[1]})
    )
    .is_err()
  );
  assert!(
    invoke(
      root,
      &json!({"op":"submit", "request":request(), "chunks":1, "size":4})
    )
    .is_err()
  );
  assert!(
    invoke(
      root,
      &json!({"op":"submit", "request":request(), "chunks":1, "size":3})
    )
    .is_err()
  );
}
#[test]
fn retired_authority_rejects_writes_but_keeps_reads() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path();
  setup(root);
  invoke(
    root,
    &json!({"op":"retire", "expected":config::load(root).unwrap(), "successor":"next"}),
  )
  .unwrap();
  assert!(
    invoke(
      root,
      &json!({"op":"chunk", "key":"task", "index":0, "bytes":[1]})
    )
    .is_err()
  );
  assert!(invoke(root, &json!({"op":"cancel", "key":"task"})).is_err());
  assert_eq!(invoke(root, &json!({"op":"list"})).unwrap(), json!([]));
}
#[test]
fn occupied_worker_cannot_be_removed_and_unfinished_authority_cannot_retire() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path();
  let mut config = worker_config(root);
  assert!(root.join("authority.json").exists());
  let store = Store::new(root);
  store.submit(request(), b"input").unwrap();
  assert!(
    invoke(
      root,
      &json!({"op":"retire", "expected":config, "successor":"next"})
    )
    .is_err()
  );
  store.reserve("task", &config, 1).unwrap().unwrap();
  let mut changed = config.clone();
  changed.machines.clear();
  assert!(
    invoke(
      root,
      &json!({"op":"config", "expected":config, "config":changed})
    )
    .is_err()
  );
  let old = config.clone();
  config.machines.get_mut("worker").unwrap().draining = true;
  invoke(
    root,
    &json!({"op":"config", "expected":old, "config":config}),
  )
  .unwrap();
}

#[test]
fn output_serves_bounded_ranges_and_rejects_past_end() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path();
  let config = worker_config(root);
  let store = Store::new(root);
  store.submit(request(), b"input").unwrap();
  let task = store.reserve("task", &config, 1).unwrap().unwrap();
  let bytes = vec![7_u8; CHUNK + 1];
  store
    .report(
      "task",
      task.attempt.as_deref().unwrap(),
      &"b".repeat(40),
      &bytes,
    )
    .unwrap();
  let first = invoke(root, &json!({"op":"output", "key":"task", "offset":0})).unwrap();
  assert_eq!(first["size"], bytes.len());
  assert_eq!(first["bytes"].as_array().unwrap().len(), CHUNK);
  assert_eq!(
    invoke(root, &json!({"op":"output", "key":"task", "offset":CHUNK})).unwrap()["bytes"],
    json!([7])
  );
  assert!(
    invoke(
      root,
      &json!({"op":"output", "key":"task", "offset":CHUNK + 2})
    )
    .is_err()
  );
}
