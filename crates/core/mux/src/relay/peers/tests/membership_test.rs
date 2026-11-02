use super::super::{
  receive,
  tests::{SECRET, auth, code, join, request, session, setup},
};
use crate::relay::config;
use serde_json::json;

#[test]
fn changed_secret_conflicts_and_removed_host_cannot_authenticate() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path();
  setup(root);
  join(root, "alice");
  let mut req = request("join");
  req["session"] = session("alice");
  req["secret"] = json!("x".repeat(32));
  req["group_key"] = json!(SECRET);
  code(&receive(root, &req), "conflict");
  config::update(root, |cfg| {
    cfg.machines.get_mut("one").unwrap().agents = vec!["claude".into()];
    Ok(())
  })
  .unwrap();
  let mut list = request("list");
  list["auth"] = auth("alice");
  code(&receive(root, &list), "unauthorized");
}

#[test]
fn unsafe_membership_identifiers_are_rejected() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path();
  setup(root);
  for field in ["id", "generation", "group", "machine"] {
    let mut req = request("join");
    req["session"] = session("alice");
    req["session"][field] = json!("../escape");
    req["secret"] = json!(SECRET);
    req["group_key"] = json!(SECRET);
    code(&receive(root, &req), "invalid");
  }
}

#[test]
fn wake_registration_requires_a_positive_foreground_pid() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path();
  setup(root);
  let mut req = request("join");
  req["session"] = session("alice");
  req["secret"] = json!(SECRET);
  req["group_key"] = json!(SECRET);
  req["session"]["wake"] = json!({"address":"one/t7","owner":"alice:gen","created_ms":123});
  code(&receive(root, &req), "invalid");
  req["session"]["wake"]["pid"] = json!(0);
  code(&receive(root, &req), "invalid");
  req["session"]["wake"]["pid"] = json!(42);
  assert!(receive(root, &req).get("result").is_some());
}
