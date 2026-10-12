use std::sync::mpsc;

use serde_json::{Value, json};
use toolu_protocol::exit::Exit;

use super::queue;
use super::tests::{parse, root};
use crate::model::Issue;
use crate::server::{Engine, Fault};

#[test]
fn queue_without_a_registered_epic_is_empty() {
  let tmp = tempfile::tempdir().expect("temp");
  let (_paths, env) = root(tmp.path());
  let outcome = queue(&parse(&["epic", "queue"]), &env);
  assert_eq!(outcome.exit, Exit::Success, "{outcome:?}");
  let doc: Value = serde_json::from_str(&outcome.stdout.expect("stdout")).expect("json");
  assert_eq!(doc, json!({"schema": 1, "repositories": []}));
}

#[test]
fn the_queue_op_answers_with_the_engine_s_queue_by_epic() {
  let tmp = tempfile::tempdir().expect("temp");
  let (paths, _env) = root(tmp.path());
  let mut engine = Engine::open(paths, None, Fault::None).expect("open");
  for (key, epic, at) in [("b", "one", 20), ("a", "two", 10)] {
    let mut issue = Issue::blank(key, epic, "/state", 0);
    "ready".clone_into(&mut issue.phase);
    "running".clone_into(&mut issue.stage);
    "o/r".clone_into(&mut issue.repo);
    issue.pr = Some(at);
    issue.queued_at_ms = Some(at);
    engine.world.issues.insert(key.to_owned(), issue);
  }
  let (tx, rx) = mpsc::channel();
  let mut waiters = Vec::new();
  let ask = |engine: &mut Engine, waiters: &mut Vec<_>, request: Value| {
    let request = json!({"protocol": crate::PROTOCOL}).as_object().cloned().map(|mut base| {
      base.extend(request.as_object().cloned().unwrap_or_default());
      Value::Object(base)
    });
    crate::dispatch::dispatch(engine, &request.expect("request"), &tx, waiters, crate::PROTOCOL);
    rx.recv().expect("reply")
  };
  let all = ask(&mut engine, &mut waiters, json!({"op": "queue"}));
  let keys: Vec<&str> = all["repositories"][0]["items"]
    .as_array()
    .expect("items")
    .iter()
    .filter_map(|item| item["key"].as_str())
    .collect();
  assert_eq!(keys, ["a", "b"]);
  let one = ask(&mut engine, &mut waiters, json!({"op": "queue", "epic": "one"}));
  assert_eq!(one["repositories"][0]["items"][0]["key"], "b");
  assert_eq!(one["repositories"][0]["items"][0]["position"], 2);
}
