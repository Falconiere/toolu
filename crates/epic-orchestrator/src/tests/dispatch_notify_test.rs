//! The `notify` socket op: validation, journaling, token dedupe, and the
//! addressed relay to one running sibling.

use serde_json::json;

use super::tests::{open, round};
use crate::server::Engine;

fn running_engine() -> (tempfile::TempDir, Engine) {
  let tmp = tempfile::tempdir().expect("temp");
  let mut engine = open(tmp.path());
  for key in ["a", "b"] {
    let issue = crate::logic::ensure_issue(&mut engine.world, key, "one", "/epic");
    issue.stage = "running".to_owned();
  }
  (tmp, engine)
}

#[test]
fn notify_rejects_unknown_kinds_and_recipients() {
  let (_tmp, mut engine) = running_engine();
  let (_, bad_kind) = round(
    &mut engine,
    &json!({"op": "notify", "token": "n0", "key": "a", "kind": "frobnicate"}),
  );
  assert_eq!(bad_kind["error"], "unknown kind frobnicate");
  let (_, no_to) = round(
    &mut engine,
    &json!({"op": "notify", "token": "n0", "key": "a", "kind": "blocked", "to": "zzz"}),
  );
  assert_eq!(no_to["error"], "unknown recipient zzz");
}

#[test]
fn notify_journals_and_applies_a_token_once() {
  let (_tmp, mut engine) = running_engine();
  let body =
    json!({"op": "notify", "token": "n1", "key": "a", "kind": "blocked", "note": "waiting"});
  let (_, value) = round(&mut engine, &body);
  assert_eq!(value["ok"], true);
  let rows = crate::journal::tail(&engine.paths.journal_dir(), engine.now).expect("tail");
  assert!(rows.iter().any(|row| row.kind == "notify"
    && row.name == "blocked"
    && row.key == "a"
    && row.token == "n1"));
  assert!(
    engine
      .world
      .attention
      .iter()
      .any(|item| item.kind == "blocked" && item.key == "a")
  );
  let lines = rows.len();
  let (_, again) = round(&mut engine, &body);
  assert_eq!(again["ok"], true);
  let after_replay = crate::journal::tail(&engine.paths.journal_dir(), engine.now).expect("tail");
  assert_eq!(after_replay.len(), lines);
}

fn relayed_prompts(engine: &Engine) -> Vec<String> {
  crate::journal::tail(&engine.paths.journal_dir(), engine.now)
    .expect("tail")
    .into_iter()
    .filter(|row| row.kind == "prompt" && row.name == "relay")
    .map(|row| row.note)
    .collect()
}

#[test]
fn notify_relays_to_one_named_sibling_at_once() {
  let (_tmp, mut engine) = running_engine();
  let (_, relayed) = round(
    &mut engine,
    &json!({"op": "notify", "token": "n2", "key": "a", "kind": "contract-changed", "note": "API renamed", "to": "b"}),
  );
  assert_eq!(relayed["ok"], true);
  assert!(engine.world.prompt_queue.is_empty());
  let prompts = relayed_prompts(&engine);
  assert_eq!(prompts.len(), 1);
  assert!(prompts[0].contains("relay from a"), "{}", prompts[0]);
  assert!(prompts[0].contains("API renamed"), "{}", prompts[0]);
  let direction = engine
    .world
    .issues
    .get("b")
    .expect("b")
    .direction
    .as_ref()
    .expect("direction");
  assert_eq!(direction.by, "relay:a");
  assert!(direction.text.contains("API renamed"), "{}", direction.text);
  let (_, to_self) = round(
    &mut engine,
    &json!({"op": "notify", "token": "n3", "key": "a", "kind": "stuck", "to": "a"}),
  );
  assert_eq!(to_self["ok"], true);
  assert_eq!(relayed_prompts(&engine).len(), 1);
}

#[test]
fn ingest_applies_a_spooled_notify() {
  let tmp = tempfile::tempdir().expect("temp");
  let mut engine = open(tmp.path());
  let spool = tmp.path().join("spool");
  std::fs::create_dir_all(&spool).expect("spool");
  let body =
    serde_json::json!({"op": "notify", "token": "n8", "key": "a", "kind": "help", "note": "stuck"});
  std::fs::write(spool.join("n8.json"), format!("{body}\n")).expect("spool file");
  crate::dispatch::ingest(&mut engine).expect("ingest");
  assert!(!spool.join("n8.json").is_file());
  assert!(
    engine
      .world
      .attention
      .iter()
      .any(|item| item.kind == "help" && item.key == "a")
  );
}

#[test]
fn notify_cools_down_a_relayed_recipient() {
  let (_tmp, mut engine) = running_engine();
  let first =
    json!({"op": "notify", "token": "n2", "key": "a", "kind": "stuck", "note": "one", "to": "b"});
  let (_, ok) = round(&mut engine, &first);
  assert_eq!(ok["ok"], true);
  assert_eq!(relayed_prompts(&engine).len(), 1);
  let repeat =
    json!({"op": "notify", "token": "n3", "key": "a", "kind": "stuck", "note": "two", "to": "b"});
  let (_, held) = round(&mut engine, &repeat);
  assert_eq!(held["ok"], true);
  assert_eq!(relayed_prompts(&engine).len(), 1);
  engine.world.now_ms = engine.world.now_ms.saturating_add(61_000);
  let after =
    json!({"op": "notify", "token": "n4", "key": "a", "kind": "stuck", "note": "three", "to": "b"});
  let (_, late) = round(&mut engine, &after);
  assert_eq!(late["ok"], true);
  assert_eq!(relayed_prompts(&engine).len(), 2);
}
