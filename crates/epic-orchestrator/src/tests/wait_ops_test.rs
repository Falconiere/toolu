use std::sync::mpsc;
use std::time::Instant;

use serde_json::{Value, json};

use super::satisfy;
use crate::PROTOCOL;
use crate::attention::{raise, reraise};
use crate::dispatch::dispatch;
use crate::epics::Epic;
use crate::paths::Paths;
use crate::server::{Engine, Fault};
use crate::socket_loop::Waiter;

fn open(root: &std::path::Path) -> Engine {
  Engine::open(Paths::at(root), None, Fault::None).expect("open")
}

fn ask(engine: &mut Engine, request: &Value, waiters: &mut Vec<Waiter>) -> mpsc::Receiver<Value> {
  let (tx, rx) = mpsc::channel();
  let _stop = dispatch(engine, request, &tx, waiters, PROTOCOL);
  rx
}

fn now(engine: &mut Engine, request: &Value) -> Value {
  ask(engine, request, &mut Vec::new())
    .try_recv()
    .unwrap_or(Value::Null)
}

fn two_epics(engine: &mut Engine) {
  for key in ["o-r-1", "o-r-9"] {
    let epic = Epic::new(key, "/state", 0, None);
    engine.world.epics.insert(key.to_owned(), epic);
  }
}

fn keys(engine: &mut Engine, requests: &[Value]) -> Vec<Value> {
  requests
    .iter()
    .map(|request| now(engine, request)["key"].clone())
    .collect()
}

#[test]
fn a_scoped_wait_skips_older_items_of_another_epic() {
  let tmp = tempfile::tempdir().expect("temp");
  let mut engine = open(tmp.path());
  two_epics(&mut engine);
  raise(&mut engine.world, "failed", "o-r-1", "paused epic");
  raise(&mut engine.world, "needs-human", "o-r-9", "mine");
  let scoped = json!({"op": "wait", "max_seconds": 0, "epic": "o-r-9"});
  let item = now(&mut engine, &scoped);
  assert_eq!(
    (&item["key"], &item["epic"]),
    (&json!("o-r-9"), &json!("o-r-9"))
  );
  assert_eq!(now(&mut engine, &scoped), json!({"state": "waiting"}));
  let any = json!({"op": "wait", "max_seconds": 0});
  assert_eq!(now(&mut engine, &any)["key"], "o-r-1");
}

#[test]
fn a_scoped_wait_also_takes_an_engine_wide_item() {
  let tmp = tempfile::tempdir().expect("temp");
  let mut engine = open(tmp.path());
  two_epics(&mut engine);
  raise(&mut engine.world, "failed", "o-r-1", "other");
  raise(&mut engine.world, "dependency-cycle", "", "everyone");
  let item = now(
    &mut engine,
    &json!({"op": "wait", "max_seconds": 0, "epic": "o-r-9"}),
  );
  assert_eq!(
    (&item["kind"], &item["epic"]),
    (&json!("dependency-cycle"), &json!(""))
  );
}

#[test]
fn an_unknown_epic_is_an_error_and_a_trailing_number_resolves() {
  let tmp = tempfile::tempdir().expect("temp");
  let mut engine = open(tmp.path());
  two_epics(&mut engine);
  raise(&mut engine.world, "failed", "o-r-1", "other");
  raise(&mut engine.world, "failed", "o-r-9", "mine");
  let unknown = json!({"op": "wait", "max_seconds": 30, "epic": "nope"});
  let mut waiters = Vec::new();
  let reply = ask(&mut engine, &unknown, &mut waiters).try_recv();
  assert_eq!(
    reply.expect("error"),
    json!({"error": "unknown epic: nope"})
  );
  assert!(waiters.is_empty());
  assert!(engine.world.attention.iter().all(|item| !item.delivered));
  let trailing = json!({"op": "wait", "max_seconds": 0, "epic": "9"});
  assert_eq!(now(&mut engine, &trailing)["key"], "o-r-9");
}

#[test]
fn blocked_waiters_each_get_their_own_epic() {
  let tmp = tempfile::tempdir().expect("temp");
  let mut engine = open(tmp.path());
  two_epics(&mut engine);
  let mut waiters = Vec::new();
  let nine = ask(
    &mut engine,
    &json!({"op": "wait", "max_seconds": 30, "epic": "o-r-9"}),
    &mut waiters,
  );
  let one = ask(
    &mut engine,
    &json!({"op": "wait", "max_seconds": 30, "epic": "o-r-1"}),
    &mut waiters,
  );
  assert_eq!(waiters.len(), 2);
  raise(&mut engine.world, "failed", "o-r-1", "first");
  raise(&mut engine.world, "failed", "o-r-9", "second");
  satisfy(&mut engine, &mut waiters).expect("satisfy");
  assert!(waiters.is_empty());
  assert_eq!(one.try_recv().expect("one")["key"], "o-r-1");
  assert_eq!(nine.try_recv().expect("nine")["key"], "o-r-9");
}

#[test]
fn a_delivered_item_stays_delivered_after_a_restart() {
  let tmp = tempfile::tempdir().expect("temp");
  let mut engine = open(tmp.path());
  raise(&mut engine.world, "failed", "i-1", "first");
  raise(&mut engine.world, "failed", "i-2", "second");
  raise(&mut engine.world, "stall", "i-3", "stalled");
  engine.flush().expect("flush");
  let blocking = json!({"op": "wait", "max_seconds": 30});
  assert_eq!(
    keys(&mut engine, &[blocking.clone(), blocking]),
    ["i-1", "i-2"]
  );
  reraise(&mut engine.world, "failed", "i-2", "again");
  engine.flush().expect("flush");
  drop(engine);
  let mut engine = open(tmp.path());
  let poll = json!({"op": "wait", "max_seconds": 0});
  let seen = keys(&mut engine, &[poll.clone(), poll.clone(), poll]);
  assert_eq!(seen, [json!("i-2"), json!("i-3"), Value::Null]);
  let rows = crate::journal::tail(&engine.paths.journal_dir(), engine.now).expect("journal");
  let tokens: Vec<&str> = rows
    .iter()
    .filter(|row| row.name == "wait-reply")
    .map(|row| row.token.as_str())
    .collect();
  assert_eq!(tokens.len(), 4, "{rows:?}");
  assert!(
    tokens.iter().all(|token| token.starts_with('a')),
    "{tokens:?}"
  );
}

#[test]
fn a_waiter_past_its_deadline_times_out_while_another_stays() {
  let tmp = tempfile::tempdir().expect("temp");
  let mut engine = open(tmp.path());
  let mut waiters = Vec::new();
  let late = ask(
    &mut engine,
    &json!({"op": "wait", "max_seconds": 30}),
    &mut waiters,
  );
  let _kept = ask(
    &mut engine,
    &json!({"op": "wait", "max_seconds": 30}),
    &mut waiters,
  );
  if let Some(first) = waiters.first_mut() {
    first.deadline = Instant::now();
  }
  satisfy(&mut engine, &mut waiters).expect("satisfy");
  assert_eq!(late.try_recv().expect("late"), json!({"state": "waiting"}));
  assert_eq!(waiters.len(), 1);
}
