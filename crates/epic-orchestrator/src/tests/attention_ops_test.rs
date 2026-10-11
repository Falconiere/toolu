use std::path::Path;
use std::sync::mpsc::channel;

use serde_json::{Value, json};

use super::{reply_ack, reply_answer};
use crate::journal::{self, Record};
use crate::model::{Ack, Report};
use crate::paths::Paths;
use crate::server::{Engine, Fault};

fn engine(root: &Path, outcome: &str) -> Engine {
  let scripts = root.join("scripts");
  std::fs::create_dir_all(&scripts).expect("scripts");
  std::fs::create_dir_all(root.join("epic/status")).expect("status");
  let script = format!(
    "cat >> '{}'\necho >> '{}'\nprintf '{{\"outcome\":\"{outcome}\"}}'\n",
    root.join("prompts").display(),
    root.join("prompts").display(),
  );
  std::fs::write(scripts.join("prompt"), script).expect("script");
  Engine::open(Paths::at(root), Some(scripts), Fault::None).expect("open")
}

fn report(engine: &mut Engine, phase: &str) {
  let state_dir = engine.paths.root.join("epic").display().to_string();
  engine
    .report(&Report {
      key: "a".to_owned(),
      epic: "one".to_owned(),
      state_dir,
      phase: phase.to_owned(),
      pr: None,
      note: "which one?".to_owned(),
    })
    .expect("report");
}

fn answer(engine: &mut Engine, request: &Value) -> Value {
  let (tx, rx) = channel();
  assert!(!reply_answer(engine, request, &tx));
  rx.recv().expect("reply")
}

fn rows(engine: &Engine) -> Vec<Record> {
  journal::tail(&engine.paths.journal_dir(), engine.now).expect("tail")
}

fn prompts(root: &Path) -> Vec<Value> {
  std::fs::read_to_string(root.join("prompts"))
    .unwrap_or_default()
    .lines()
    .map(|line| serde_json::from_str(line).expect("prompt json"))
    .collect()
}

#[test]
fn answer_reaches_the_prompt_script_and_the_next_report_closes_it() {
  let tmp = tempfile::tempdir().expect("temp");
  let mut engine = engine(tmp.path(), "sent");
  report(&mut engine, "needs-human");
  let token = engine.world.attention[0].token.clone();
  let request = json!({"op": "answer", "key": "a", "text": "take B", "by": "user"});
  assert_eq!(answer(&mut engine, &request), json!({"ok": true}));
  assert_eq!(prompts(tmp.path()), [json!({"key": "a", "text": "take B"})]);
  assert_eq!(engine.world.attention[0].state, Ack::Answered);
  let journal = rows(&engine);
  let answered: Vec<(&str, &str, &str)> = journal
    .iter()
    .filter(|row| row.token == token && matches!(row.name.as_str(), "answer" | "answered"))
    .map(|row| (row.kind.as_str(), row.name.as_str(), row.note.as_str()))
    .collect();
  assert_eq!(
    answered,
    [
      ("prompt", "answer", "take B"),
      ("judgment", "answered", "by=user")
    ]
  );
  report(&mut engine, "execution");
  assert_eq!(engine.world.attention.len(), 0);
  let closed = rows(&engine)
    .into_iter()
    .find(|row| row.name == "closed")
    .expect("closed");
  assert_eq!(closed.token, token);
  assert!(
    closed.note.starts_with("kind=needs-human waited_ms="),
    "{}",
    closed.note
  );
}

#[test]
fn answer_without_a_live_item_calls_no_prompt() {
  let tmp = tempfile::tempdir().expect("temp");
  let mut engine = engine(tmp.path(), "sent");
  let request = json!({"op": "answer", "key": "a", "text": "take B"});
  assert_eq!(
    answer(&mut engine, &request),
    json!({"error": "no open attention for a"})
  );
  assert_eq!(prompts(tmp.path()), [] as [Value; 0]);
}

#[test]
fn answer_not_delivered_leaves_the_item_open() {
  let tmp = tempfile::tempdir().expect("temp");
  let mut engine = engine(tmp.path(), "failed");
  report(&mut engine, "needs-human");
  let request = json!({"op": "answer", "key": "a", "text": "take B"});
  assert_eq!(
    answer(&mut engine, &request),
    json!({"error": "worker prompt not delivered: prompt failed"})
  );
  assert_eq!(engine.world.attention[0].state, Ack::Open);
  assert!(!rows(&engine).iter().any(|row| row.name == "answered"));
}

#[test]
fn ack_journals_who_acknowledged() {
  let tmp = tempfile::tempdir().expect("temp");
  let mut engine = engine(tmp.path(), "sent");
  report(&mut engine, "failed");
  let (tx, rx) = channel();
  let request = json!({"op": "ack", "key": "a", "by": "cursor"});
  assert_eq!(reply_ack(&mut engine, &request, &tx), Ok(false));
  assert_eq!(rx.recv().expect("reply"), json!({"ok": true}));
  assert_eq!(engine.world.attention[0].state, Ack::Acknowledged);
  let acked = rows(&engine)
    .into_iter()
    .find(|row| row.name == "acknowledged")
    .expect("acknowledged");
  assert_eq!(acked.note, "by=cursor");
}
