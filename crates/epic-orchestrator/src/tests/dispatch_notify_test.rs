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
  assert_eq!(relayed_prompts(&engine).len(), 0);
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
fn notify_scrubs_secrets_before_a_relay() {
  let (_tmp, mut engine) = running_engine();
  let (_, value) = round(
    &mut engine,
    &json!({"op": "notify", "token": "n5", "key": "a", "kind": "help", "note": "leaked ghp_secret", "to": "b"}),
  );
  assert_eq!(value["ok"], true);
  let direction = engine
    .world
    .issues
    .get("b")
    .expect("b")
    .direction
    .as_ref()
    .expect("direction");
  assert!(!direction.text.contains("ghp_secret"), "{}", direction.text);
  assert!(direction.text.contains("[redacted]"), "{}", direction.text);
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

fn write_spool(
  tmp: &tempfile::TempDir,
  token: &str,
  body: &serde_json::Value,
) -> std::path::PathBuf {
  let spool = tmp.path().join("spool");
  std::fs::create_dir_all(&spool).expect("spool");
  let path = spool.join(format!("{token}.json"));
  std::fs::write(&path, format!("{body}\n")).expect("spool file");
  path
}

#[test]
fn a_journaled_notify_replay_without_a_spool_marker_has_no_second_effect() {
  let (tmp, mut engine) = running_engine();
  let body = json!({"op": "notify", "token": "n-replay", "key": "a", "kind": "help", "note": "same", "to": "b"});
  let path = write_spool(&tmp, "n-replay", &body);
  crate::dispatch_ops::apply_notify(&mut engine, &body).expect("valid notice");
  engine.flush().expect("durable notify before spool marker");
  let before = crate::journal::tail(&engine.paths.journal_dir(), engine.now).expect("tail");
  assert_eq!(before.iter().filter(|row| row.kind == "notify").count(), 1);
  let (_, duplicate) = round(&mut engine, &body);
  assert_eq!(duplicate["ok"], true);
  assert_eq!(
    crate::journal::tail(&engine.paths.journal_dir(), engine.now)
      .expect("tail")
      .len(),
    before.len()
  );
  drop(engine);

  let mut restarted = open(tmp.path());
  for key in ["a", "b"] {
    let issue = crate::logic::ensure_issue(&mut restarted.world, key, "one", "/epic");
    issue.stage = "running".to_owned();
  }
  crate::dispatch::ingest(&mut restarted).expect("replay");
  assert!(!path.is_file());
  let after = crate::journal::tail(&restarted.paths.journal_dir(), restarted.now).expect("tail");
  assert_eq!(after.iter().filter(|row| row.kind == "notify").count(), 1);
  assert_eq!(
    after.iter().filter(|row| row.kind == "judgment").count(),
    before.iter().filter(|row| row.kind == "judgment").count()
  );
  assert_eq!(
    after
      .iter()
      .filter(|row| row.kind == "prompt" && row.name == "relay")
      .count(),
    before
      .iter()
      .filter(|row| row.kind == "prompt" && row.name == "relay")
      .count()
  );
}

#[test]
fn an_old_journaled_notify_still_suppresses_spool_replay() {
  let (tmp, mut engine) = running_engine();
  let body = json!({"op": "notify", "token": "n-old", "key": "a", "kind": "help", "to": "b"});
  let path = write_spool(&tmp, "n-old", &body);
  crate::dispatch_ops::apply_notify(&mut engine, &body).expect("valid notice");
  engine.flush().expect("durable notify");
  let journal = engine.paths.journal_dir();
  let old = engine.now - std::time::Duration::from_hours(72);
  std::fs::rename(
    crate::journal::file_for(&journal, engine.now),
    crate::journal::file_for(&journal, old),
  )
  .expect("move retained journal day");
  drop(engine);

  let mut restarted = open(tmp.path());
  for key in ["a", "b"] {
    let issue = crate::logic::ensure_issue(&mut restarted.world, key, "one", "/epic");
    issue.stage = "running".to_owned();
  }
  crate::dispatch::ingest(&mut restarted).expect("replay older token");
  assert!(!path.is_file());
  let mut rows = Vec::new();
  crate::journal::each(&journal, "", |row| {
    rows.push(row);
    Ok(())
  })
  .expect("retained journal");
  assert_eq!(rows.iter().filter(|row| row.kind == "notify").count(), 1);
  assert_eq!(rows.iter().filter(|row| row.kind == "judgment").count(), 1);
  assert_eq!(
    rows
      .iter()
      .filter(|row| row.kind == "prompt" && row.name == "relay")
      .count(),
    1
  );
}

#[test]
fn a_refused_notify_drops_its_spool_file() {
  let (tmp, mut engine) = running_engine();
  let body = json!({"op": "notify", "token": "n6", "key": "a", "kind": "help", "to": "zzz"});
  let path = write_spool(&tmp, "n6", &body);
  let (_, refused) = round(&mut engine, &body);
  assert_eq!(refused["error"], "unknown recipient zzz");
  assert!(!path.is_file(), "a restart would replay the refused notice");
}

#[test]
fn ingest_sets_a_refused_notify_aside_and_keeps_starting() {
  let (tmp, mut engine) = running_engine();
  let body = json!({"op": "notify", "token": "n7", "key": "a", "kind": "help", "to": "zzz"});
  let path = write_spool(&tmp, "n7", &body);
  crate::dispatch::ingest(&mut engine).expect("a refused notice must not fail ingest");
  assert!(!path.is_file());
  assert!(path.with_extension("bad").is_file());
  assert_eq!(engine.world.attention.len(), 0);
}

#[test]
fn notify_does_not_relay_across_epics() {
  let (_tmp, mut engine) = running_engine();
  let other = crate::logic::ensure_issue(&mut engine.world, "c", "two", "/other");
  other.stage = "running".to_owned();
  let (_, refused) = round(
    &mut engine,
    &json!({"op": "notify", "token": "n9", "key": "a", "kind": "help", "to": "c"}),
  );
  assert_eq!(refused["error"], "unknown recipient c");
  assert_eq!(relayed_prompts(&engine).len(), 0);
}
