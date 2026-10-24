use serde_json::{Value, json};

use super::tests::{answer, engine, prompts, report, rows};

#[test]
fn answer_for_an_unknown_issue_calls_no_prompt() {
  let tmp = tempfile::tempdir().expect("temp");
  let mut engine = engine(tmp.path(), "sent");
  let request = json!({"op": "answer", "key": "a", "text": "take B"});
  assert_eq!(
    answer(&mut engine, &request),
    json!({"error": "no live agent for a"})
  );
  assert_eq!(prompts(tmp.path()), [] as [Value; 0]);
  assert!(rows(&engine).iter().all(|row| row.name != "answer"));
}

#[test]
fn answer_for_a_merged_issue_calls_no_prompt() {
  let tmp = tempfile::tempdir().expect("temp");
  let mut engine = engine(tmp.path(), "sent");
  report(&mut engine, "execution");
  engine.world.issues.get_mut("a").expect("issue").stage = "merged".to_owned();
  let request = json!({"op": "answer", "key": "a", "text": "take B"});
  assert_eq!(
    answer(&mut engine, &request),
    json!({"error": "no live agent for a"})
  );
  assert_eq!(prompts(tmp.path()), [] as [Value; 0]);
}

#[test]
fn answer_redirects_a_live_worker_with_no_open_item() {
  let tmp = tempfile::tempdir().expect("temp");
  let mut engine = engine(tmp.path(), "sent");
  report(&mut engine, "execution");
  let request = json!({"op": "answer", "key": "a", "text": "NEW DIRECTION", "by": "user"});
  assert_eq!(answer(&mut engine, &request), json!({"ok": true}));
  assert_eq!(prompts(tmp.path()).len(), 1);
  let sent: Vec<(String, String)> = rows(&engine)
    .into_iter()
    .filter(|row| row.name == "answer")
    .map(|row| (row.key, row.note))
    .collect();
  assert_eq!(sent, [("a".to_owned(), "NEW DIRECTION".to_owned())]);
  let direction = engine.world.issues["a"]
    .direction
    .clone()
    .expect("direction");
  assert_eq!(
    (direction.text.as_str(), direction.by.as_str()),
    ("NEW DIRECTION", "user")
  );
}

#[test]
fn a_redirect_not_delivered_records_nothing() {
  let tmp = tempfile::tempdir().expect("temp");
  let mut engine = engine(tmp.path(), "failed");
  report(&mut engine, "execution");
  let request = json!({"op": "answer", "key": "a", "text": "go"});
  assert_eq!(
    answer(&mut engine, &request),
    json!({"error": "worker prompt not delivered: prompt failed"})
  );
  assert!(engine.world.issues["a"].direction.is_none());
  assert!(rows(&engine).iter().all(|row| row.name != "answer"));
}
