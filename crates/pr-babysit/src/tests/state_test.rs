use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use super::{load, sibling, slot, valid_repo, write_json};
use crate::fixture::path as fixture;

#[test]
fn repos_follow_githubs_owner_and_name_rules() {
  for good in ["Falconiere/toolu", "a/b", "o-1/_r.x-y", "A_b/c"] {
    assert!(valid_repo(good), "{good}");
  }
  for bad in [
    "", "toolu", "/r", "o/", "-o/r", "o/.r", "o/r/x", "o r/x", "o/r x",
  ] {
    assert!(!valid_repo(bad), "{bad}");
  }
}

#[test]
fn slots_and_siblings_are_derived_from_the_state_path() {
  assert_eq!(slot("Falconiere/Toolu", 165), "falconiere-toolu-165");
  assert_eq!(
    sibling(Path::new("/tmp/pr-babysit-o-r-1.json"), ".snapshot.json"),
    PathBuf::from("/tmp/pr-babysit-o-r-1.snapshot.json")
  );
  assert_eq!(
    sibling(Path::new("/tmp/state"), ".lock"),
    PathBuf::from("/tmp/state.lock")
  );
}

#[test]
fn the_shipped_initial_state_loads() {
  let state = load(&fixture("states/toolu-165-initial.json")).unwrap();
  assert_eq!(state["repo"], "Falconiere/toolu");
  assert_eq!(state["number"], 165);
}

fn load_error(body: &str) -> (String, Value) {
  let dir = tempfile::tempdir().unwrap();
  let path = dir.path().join("s.json");
  std::fs::write(&path, body).unwrap();
  let err = load(&path).unwrap_err();
  (err.message, Value::Object(err.extra))
}

#[test]
fn malformed_states_are_refused_with_their_reason() {
  let missing = load(Path::new("/nonexistent/s.json")).unwrap_err();
  assert_eq!(missing.code, "state_malformed");
  assert!(missing.message.contains("run toolu babysit tick first"));
  assert!(load_error("{").0.contains("not valid JSON"));
  let (message, extra) = load_error(r#"{"version":1,"repo":"o/r","number":1}"#);
  assert!(message.contains("not a version-2"));
  assert_eq!(extra["version"], 1);
  assert_eq!(load_error("[]").1["version"], Value::Null);
  assert!(
    load_error(r#"{"version":2,"number":1}"#)
      .0
      .contains("version-2")
  );
  assert!(
    load_error(r#"{"version":2,"repo":"o/r","number":"1"}"#)
      .0
      .contains("version-2")
  );
  assert!(
    load_error(r#"{"version":2,"repo":"bad","number":1}"#)
      .0
      .contains("owner/name")
  );
  assert!(
    load_error(r#"{"version":2,"repo":"o/r","number":1.5}"#)
      .0
      .contains("not an integer")
  );
}

#[test]
fn writes_are_whole_documents_and_report_failure() {
  let dir = tempfile::tempdir().unwrap();
  let path = dir.path().join("deep/s.json");
  write_json(&path, &json!({"version":2}), "state").unwrap();
  assert_eq!(std::fs::read_to_string(&path).unwrap(), "{\"version\":2}\n");
  let blocked = dir.path().join("deep/s.json/child.json");
  let err = write_json(&blocked, &json!({}), "snapshot_out").unwrap_err();
  assert_eq!(err.code, "invalid_json");
  assert_eq!(err.extra["source"], "snapshot_out");
}

#[test]
fn local_only_retired_fixer_state_is_refused_without_rewriting() {
  let dir = tempfile::tempdir().unwrap();
  for patch in [
    json!({"herdrWorktree":{"multiplexer":"herdr-app", "path":"/work"}}),
    json!({"herdrWorktree":{"remote":{"machine":"worker", "path":"/worker/task"}}}),
    json!({"relayEnabled":true}),
    json!({"fixer":{"groups":[{"seq":1,"status":"running","hcAddress":"t1"}]}}),
    json!({"fixer":{"groups":[{"seq":1,"relayRequest":{"key":"attempt-1"}}]}}),
    json!({"fixer":{"remoteRemoval":{"remote":{"machine":"worker"},"head":"commit"}}}),
    json!({"remoteHistory":[{"machine":"worker","path":"/worker/task"}]}),
  ] {
    let mut body = json!({"version":2,"repo":"Falconiere/toolu","number":165});
    body
      .as_object_mut()
      .unwrap()
      .extend(patch.as_object().unwrap().clone());
    let bytes = serde_json::to_vec_pretty(&body).unwrap();
    let path = dir.path().join("retired.json");
    std::fs::write(&path, &bytes).unwrap();
    let result = load(&path);
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    let error = result.expect_err("retired fixer must not become a local fixer");
    assert!(error.message.contains("unsupported"), "{error}");
    assert!(
      error.message.contains(&path.display().to_string()),
      "{error}"
    );
    assert!(error.message.contains("pre-removal"), "{error}");
  }
}

#[test]
fn local_only_neutral_legacy_relay_metadata_keeps_local_state_readable() {
  let dir = tempfile::tempdir().unwrap();
  let path = dir.path().join("local.json");
  let body = json!({
    "version":2, "repo":"Falconiere/toolu", "number":165,
    "relayEnabled":false, "relayMachine":null, "remoteHistory":[],
    "herdrWorktree":{"multiplexer":"herdr", "remote":null},
    "fixer":{"remoteRemoval":null, "groups":[{"agent":"local-fixer", "status":"done"}]},
  });
  let bytes = serde_json::to_vec_pretty(&body).unwrap();
  std::fs::write(&path, &bytes).unwrap();
  assert_eq!(load(&path).unwrap(), body);
  assert_eq!(std::fs::read(&path).unwrap(), bytes);
}
