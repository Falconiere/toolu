//! Durable notify tokens across real spool and journal failure boundaries.

use serde_json::json;

use super::tests::{open, round};

fn spool(tmp: &tempfile::TempDir, token: &str, body: &serde_json::Value) -> std::path::PathBuf {
  let dir = tmp.path().join("spool");
  std::fs::create_dir_all(&dir).expect("spool dir");
  let path = dir.join(format!("{token}.json"));
  std::fs::write(&path, format!("{body}\n")).expect("spool file");
  path
}

#[test]
fn a_notify_lookup_error_keeps_the_spool_until_it_can_be_replayed() {
  let tmp = tempfile::tempdir().expect("temp");
  let mut engine = open(tmp.path());
  let body = json!({"op": "notify", "token": "n-read", "key": "a", "kind": "help"});
  let path = spool(&tmp, "n-read", &body);
  let journal = engine.paths.journal_dir();
  std::fs::write(&journal, "not a directory").expect("block journal read");
  let (_, refused) = round(&mut engine, &body);
  assert!(refused["error"].is_string());
  assert!(path.is_file());
  assert_eq!(engine.world.attention.len(), 0);
  std::fs::remove_file(&journal).expect("restore journal");
  crate::dispatch::ingest(&mut engine).expect("replay after read failure");
  assert!(!path.is_file());
  let rows = crate::journal::tail(&journal, engine.now).expect("tail");
  assert_eq!(rows.iter().filter(|row| row.kind == "notify").count(), 1);
}

#[test]
fn a_notify_write_error_keeps_the_spool_until_restart() {
  let tmp = tempfile::tempdir().expect("temp");
  let mut engine = open(tmp.path());
  let body = json!({"op": "notify", "token": "n-write", "key": "a", "kind": "help"});
  let path = spool(&tmp, "n-write", &body);
  std::fs::create_dir_all(engine.paths.journal_lock()).expect("block journal lock");
  let (_, refused) = round(&mut engine, &body);
  assert_eq!(refused["error"], "journal-busy");
  assert!(path.is_file());
  assert_eq!(
    crate::journal::tail(&engine.paths.journal_dir(), engine.now)
      .expect("tail")
      .len(),
    0
  );
  std::fs::remove_dir(engine.paths.journal_lock()).expect("restore lock");
  drop(engine);
  let mut restarted = open(tmp.path());
  crate::dispatch::ingest(&mut restarted).expect("replay after write failure");
  assert!(!path.is_file());
  let rows = crate::journal::tail(&restarted.paths.journal_dir(), restarted.now).expect("tail");
  assert_eq!(rows.iter().filter(|row| row.kind == "notify").count(), 1);
}

#[test]
fn a_spool_marker_error_preserves_the_durable_notify_for_replay() {
  let tmp = tempfile::tempdir().expect("temp");
  let mut engine = open(tmp.path());
  let body = json!({"op": "notify", "token": "n-marker", "key": "a", "kind": "help"});
  let path = spool(&tmp, "n-marker", &body);
  crate::dispatch_ops::apply_notify(&mut engine, &body).expect("notice");
  engine.flush().expect("durable notify");
  std::fs::create_dir_all(engine.paths.journal_lock()).expect("block marker lock");
  assert_eq!(
    crate::dispatch::remember_spool(&engine, &body).expect_err("marker must fail"),
    "journal-busy"
  );
  assert!(path.is_file());
  std::fs::remove_dir(engine.paths.journal_lock()).expect("restore lock");
  drop(engine);
  let mut restarted = open(tmp.path());
  crate::dispatch::ingest(&mut restarted).expect("replay after marker failure");
  assert!(!path.is_file());
  let rows = crate::journal::tail(&restarted.paths.journal_dir(), restarted.now).expect("tail");
  assert_eq!(rows.iter().filter(|row| row.kind == "notify").count(), 1);
}

#[test]
fn notify_tokens_are_global_and_report_deduplication_stays_on_markers() {
  let tmp = tempfile::tempdir().expect("temp");
  let mut engine = open(tmp.path());
  let body = json!({"op": "notify", "token": "n-scope", "key": "a", "kind": "help"});
  crate::dispatch_ops::apply_notify(&mut engine, &body).expect("notice");
  engine.flush().expect("durable notify");
  assert!(crate::dispatch::spool_seen(&engine, &body).expect("notify lookup"));
  assert!(
    crate::dispatch::spool_seen(
      &engine,
      &json!({"op": "notify", "token": "n-scope", "key": "b"})
    )
    .expect("global token")
  );
  assert!(
    !crate::dispatch::spool_seen(&engine, &json!({"op": "report", "token": "n-scope"}))
      .expect("report lookup")
  );
}

#[test]
fn distinct_notify_tokens_with_identical_text_keep_the_relay_cooldown() {
  let tmp = tempfile::tempdir().expect("temp");
  let mut engine = open(tmp.path());
  for key in ["a", "b"] {
    let issue = crate::logic::ensure_issue(&mut engine.world, key, "one", "/epic");
    issue.stage = "running".to_owned();
  }
  let body = |token| json!({"op": "notify", "token": token, "key": "a", "kind": "help", "note": "same", "to": "b"});
  for token in ["n-first", "n-first", "n-second"] {
    assert_eq!(round(&mut engine, &body(token)).1["ok"], true);
  }
  engine.world.now_ms = engine.world.now_ms.saturating_add(61_000);
  assert_eq!(round(&mut engine, &body("n-third")).1["ok"], true);
  let rows = crate::journal::tail(&engine.paths.journal_dir(), engine.now).expect("tail");
  assert_eq!(rows.iter().filter(|row| row.kind == "notify").count(), 3);
  assert_eq!(
    rows
      .iter()
      .filter(|row| row.kind == "prompt" && row.name == "relay")
      .count(),
    2
  );
}
