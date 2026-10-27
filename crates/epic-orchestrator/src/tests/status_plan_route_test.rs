//! `plan` and `plan_summary` through a real engine's HTTP listener and control socket.

use std::fs::File;
use std::path::Path;
use std::time::Duration;

use serde_json::{Value, json};
use toolu_protocol::host::Host;
use toolu_runtime::config::secrets::Secrets;

use super::plan;
use crate::PROTOCOL;
use crate::client::exchange_retry;
use crate::disk::write_value;
use crate::fleet_rig::{Engine, config, free_port, get, get_json, secrets_of};
use crate::plan_rig::{STEPS, Tree};

const EPIC: &str = "repo-649";
const KEY: &str = "repo-652";

/// A running engine whose issue `KEY` is a `host` worker in `tree`.
fn engine(root: &Path, tree: &Tree, host: Host, secrets: Secrets) -> Engine {
  let engine = Engine::start(root, EPIC, KEY, config(free_port(), Vec::new(), secrets));
  let record = json!({"key": KEY, "kind": host.name(), "worktree": tree.path()});
  write_value(&root.join(EPIC).join(format!("issues/{KEY}.json")), &record).expect("issue record");
  engine
}

fn socket_status(engine: &Engine) -> Value {
  exchange_retry(
    &engine.paths,
    PROTOCOL,
    &json!({"op": "status", "epic": KEY}),
  )
  .expect("status")
}

#[test]
fn the_issue_route_and_socket_status_show_the_ledgers_plan() {
  let temp = tempfile::tempdir().expect("temp root");
  let tree = Tree::new();
  let ledger = tree.ledger(Host::Claude, STEPS);
  let engine = engine(temp.path(), &tree, Host::Claude, Secrets::none());
  let issue = get_json(engine.addr, &format!("/api/v1/issues/{KEY}"));
  assert_eq!(issue["plan"], plan(Some(tree.path()), Some("claude")));
  assert_eq!(
    issue["plan"]["summary"]["green"],
    ledger["summary"]["green"]
  );
  assert_eq!(issue["plan"]["next"], "s2");
  assert_eq!(socket_status(&engine)["plan"], issue["plan"]);
  engine.stop();
}

#[test]
fn a_missing_or_corrupt_ledger_keeps_the_rest_of_the_document() {
  let temp = tempfile::tempdir().expect("temp root");
  let tree = Tree::new();
  let engine = engine(temp.path(), &tree, Host::Claude, Secrets::none());
  let path = format!("/api/v1/issues/{KEY}");
  let missing = get_json(engine.addr, &path);
  assert_eq!(missing["plan"], Value::Null);
  tree.ledger(Host::Claude, STEPS);
  std::fs::write(tree.file(Host::Claude, "feat/x"), "{").expect("corrupt ledger");
  let corrupt = get_json(engine.addr, &path);
  assert_eq!(corrupt["plan"], json!({"error": "unreadable"}));
  for doc in [&missing, &corrupt] {
    assert_eq!(
      (doc["key"].clone(), doc["phase"].clone()),
      (json!(KEY), json!("execution"))
    );
    assert_eq!(doc["phase_history"][0]["phase"], "execution");
  }
  let epic = get_json(engine.addr, &format!("/api/v1/epics/{EPIC}"));
  assert_eq!(
    epic["issues"][0]["plan_summary"],
    json!({"error": "unreadable"})
  );
  engine.stop();
}

#[test]
fn the_epic_route_serves_a_muse_plan_summary_cached_by_mtime() {
  let temp = tempfile::tempdir().expect("temp root");
  let tree = Tree::new();
  tree.ledger(Host::Muse, STEPS);
  let engine = engine(temp.path(), &tree, Host::Muse, Secrets::none());
  let path = format!("/api/v1/epics/{EPIC}");
  let summary = |engine: &Engine| get_json(engine.addr, &path)["issues"][0]["plan_summary"].clone();
  let first = summary(&engine);
  assert_eq!(
    (first["total"].clone(), first["next"].clone()),
    (json!(2), json!("s2"))
  );
  let file = tree.file(Host::Muse, "feat/x");
  let before = std::fs::metadata(&file)
    .expect("meta")
    .modified()
    .expect("mtime");
  tree.edit(Host::Muse, |ledger| ledger["summary"]["total"] = json!(7));
  File::options()
    .write(true)
    .open(&file)
    .expect("open")
    .set_modified(before)
    .expect("old mtime");
  assert_eq!(summary(&engine), first);
  let later = before + Duration::from_secs(5);
  File::options()
    .write(true)
    .open(&file)
    .expect("open")
    .set_modified(later)
    .expect("new mtime");
  assert_eq!(summary(&engine)["total"], 7);
  engine.stop();
}

#[test]
fn private_fields_never_leak_and_secret_titles_are_redacted() {
  let temp = tempfile::tempdir().expect("temp root");
  let (sentinel, secret) = ("PLAN-SECRET-CANARY", "PLAN-TITLE-SECRET-1");
  let tokens = json!({"peer": secret}).to_string();
  let secrets = secrets_of(temp.path(), &[("TOOLU_EPIC_PEER_TOKENS", &tokens)]);
  let tree = Tree::new();
  tree.ledger(Host::Claude, STEPS);
  tree.edit(Host::Claude, |ledger| {
    let step = &mut ledger["steps"][1];
    for key in ["check", "evidence_tail", "input"] {
      step[key] = json!(sentinel);
    }
    step["title"] = json!(format!("deploy with {secret}"));
  });
  let engine = engine(temp.path(), &tree, Host::Claude, secrets);
  let (_, issue) = get(engine.addr, &format!("/api/v1/issues/{KEY}"));
  let (_, epic) = get(engine.addr, &format!("/api/v1/epics/{EPIC}"));
  let socket = socket_status(&engine).to_string();
  for body in [&issue, &epic, &socket] {
    assert!(!body.contains(sentinel), "{body}");
  }
  assert!(!issue.contains(secret), "{issue}");
  assert!(issue.contains("deploy with <redacted>"), "{issue}");
  engine.stop();
}
