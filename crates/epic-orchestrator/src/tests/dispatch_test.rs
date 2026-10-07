use std::sync::mpsc;
use std::time::Instant;

use serde_json::{Value, json};

use super::{dispatch, expire, ingest, report_from, satisfy};
use crate::PROTOCOL;
use crate::paths::Paths;
use crate::server::{Engine, Fault};

#[test]
fn a_report_request_copies_its_phase() {
  let report = report_from(&json!({
    "key": "a",
    "epic": "one",
    "state_dir": "/tmp",
    "phase": "ready",
  }));
  assert_eq!(report.key, "a");
  assert_eq!(report.phase, "ready");
  assert_eq!(report.epic, "one");
  let empty = report_from(&json!({}));
  assert_eq!(empty.key, "");
  assert!(empty.pr.is_none());
}

#[test]
fn control_ops_stop_replace_or_reject() {
  let tmp = tempfile::tempdir().expect("temp");
  let mut engine = open(tmp.path());
  let (stop, value) = round(&mut engine, &json!({"op": "status", "protocol": 9}));
  assert!(!stop);
  assert_eq!(value["error"], "protocol");
  let (stop, value) = round(&mut engine, &json!({"op": "nope"}));
  assert!(!stop);
  assert_eq!(value["error"], "unknown op");
  let (stop, value) = round(&mut engine, &json!({"op": "replace", "protocol": 9}));
  assert!(stop);
  assert_eq!(value["ok"], true);
  assert!(engine.world.draining);
  let (stop, value) = round(&mut engine, &json!({"op": "stop"}));
  assert!(stop);
  assert_eq!(value["ok"], true);
}

#[test]
fn status_pause_ack_and_answer_reply() {
  let tmp = tempfile::tempdir().expect("temp");
  let mut engine = open(tmp.path());
  let (stop, value) = round(&mut engine, &json!({"op": "status", "epic": "one"}));
  assert!(!stop);
  assert_eq!(value["engine"], "running");
  let (stop, value) = round(&mut engine, &json!({"op": "pause", "epic": "one"}));
  assert!(!stop);
  assert_eq!(value["ok"], true);
  let (stop, value) = round(&mut engine, &json!({"op": "resume"}));
  assert!(!stop);
  assert_eq!(value["ok"], true);
  let (stop, value) = round(&mut engine, &json!({"op": "ack", "key": "a"}));
  assert!(!stop);
  assert_eq!(value["ok"], true);
  let (stop, value) = round(
    &mut engine,
    &json!({"op": "answer", "key": "a", "text": "hi"}),
  );
  assert!(!stop);
  assert_eq!(value["ok"], true);
}

#[test]
fn events_and_reports_are_journaled() {
  let tmp = tempfile::tempdir().expect("temp");
  let epic = tmp.path().join("epic");
  std::fs::create_dir_all(epic.join("status")).expect("status");
  std::fs::create_dir_all(tmp.path().join("spool")).expect("spool");
  std::fs::write(tmp.path().join("spool/t1.json"), "{}\n").expect("spool file");
  let mut engine = open(tmp.path());
  for event in ["blocked", "host-limited", "gone", "nope"] {
    let (stop, value) = round(
      &mut engine,
      &json!({"op": "event", "key": "a", "event": event}),
    );
    assert!(!stop);
    assert_eq!(value["ok"], true);
  }
  let (stop, value) = round(
    &mut engine,
    &json!({"op": "report", "key": "a", "phase": "x", "state_dir": epic.display().to_string()}),
  );
  assert!(!stop);
  assert_eq!(value["ok"], true);
  let (stop, value) = round(
    &mut engine,
    &json!({
      "op": "report",
      "token": "t1",
      "key": "a",
      "epic": "one",
      "state_dir": epic.display().to_string(),
      "phase": "execution",
      "pr": 4,
      "note": "n",
    }),
  );
  assert!(!stop);
  assert_eq!(value["ok"], true);
  assert!(!tmp.path().join("spool/t1.json").is_file());
}

#[test]
fn wait_replies_expires_or_returns_a_judgment() {
  let tmp = tempfile::tempdir().expect("temp");
  let mut engine = open(tmp.path());
  let (stop, value) = round(&mut engine, &json!({"op": "wait", "max_seconds": 0}));
  assert!(!stop);
  assert_eq!(value["state"], "waiting");
  let (tx, rx) = mpsc::channel();
  let mut waiters = Vec::new();
  let stop = dispatch(
    &mut engine,
    &json!({"op": "wait", "max_seconds": 30}),
    tx,
    &mut waiters,
    PROTOCOL,
  )
  .expect("wait");
  assert!(!stop);
  assert!(rx.try_recv().is_err());
  assert_eq!(waiters.len(), 1);
  waiters[0].deadline = Instant::now();
  expire(&mut engine, &mut waiters);
  assert_eq!(rx.recv().expect("expired")["state"], "waiting");
  satisfy(&mut engine, &mut waiters);
  crate::schedule::note_event(&mut engine.world, "a", "blocked");
  let (stop, value) = round(&mut engine, &json!({"op": "wait", "max_seconds": 30}));
  assert!(!stop);
  assert_eq!(value["kind"], "blocked");
}

#[test]
fn ingest_applies_a_new_spool_and_skips_a_seen_token() {
  let tmp = tempfile::tempdir().expect("temp");
  let epic = tmp.path().join("epic");
  std::fs::create_dir_all(epic.join("status")).expect("status");
  let mut engine = open(tmp.path());
  ingest(&mut engine).expect("missing spool");
  let spool = tmp.path().join("spool");
  std::fs::create_dir_all(&spool).expect("spool");
  std::fs::write(spool.join("bad.json"), "not-json\n").expect("bad");
  assert!(ingest(&mut engine).is_err());
  std::fs::remove_file(spool.join("bad.json")).expect("remove bad");
  std::fs::write(spool.join("empty.json"), "{}\n").expect("empty");
  ingest(&mut engine).expect("empty token");
  assert!(!spool.join("empty.json").is_file());
  let body = json!({
    "token": "t2",
    "key": "a",
    "epic": "one",
    "state_dir": epic.display().to_string(),
    "phase": "execution",
  });
  std::fs::write(spool.join("t2.json"), format!("{body}\n")).expect("t2");
  ingest(&mut engine).expect("apply");
  std::fs::write(spool.join("t2.json"), format!("{body}\n")).expect("again");
  ingest(&mut engine).expect("seen");
  assert!(!spool.join("t2.json").is_file());
  std::fs::remove_dir_all(&spool).expect("clear");
  std::fs::write(&spool, "x").expect("file");
  assert!(ingest(&mut engine).is_err());
}

fn open(root: &std::path::Path) -> Engine {
  Engine::open(Paths::at(root), None, Fault::None).expect("open")
}

fn round(engine: &mut Engine, request: &Value) -> (bool, Value) {
  let (tx, rx) = mpsc::channel();
  let mut waiters = Vec::new();
  let stop = dispatch(engine, request, tx, &mut waiters, PROTOCOL).expect("dispatch");
  (stop, rx.try_recv().unwrap_or(Value::Null))
}
