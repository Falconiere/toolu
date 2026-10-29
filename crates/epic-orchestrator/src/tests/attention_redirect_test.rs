//! Redirect answers and uncertain-delivery classifiers for `answer`.

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
    .filter(|row| row.kind == "prompt" && row.name == "answer")
    .map(|row| (row.key, row.note))
    .collect();
  assert_eq!(sent, [("a".to_owned(), "NEW DIRECTION".to_owned())]);
  let action: Vec<(String, String)> = rows(&engine)
    .into_iter()
    .filter(|row| row.kind == "action" && row.name == "answer")
    .map(|row| (row.token, row.note))
    .collect();
  assert_eq!(action, [("".to_owned(), "by=user".to_owned())]);
  let directed: Vec<String> = rows(&engine)
    .into_iter()
    .filter(|row| row.name == "directed")
    .map(|row| row.note)
    .collect();
  assert_eq!(directed, ["by=user"]);
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

#[test]
fn an_uncertain_answer_replies_ok_journals_it_and_leaves_attention_open() {
  let tmp = tempfile::tempdir().expect("temp");
  let mut engine = engine(tmp.path(), "uncertain");
  report(&mut engine, "needs-human");
  let request = json!({"op": "answer", "key": "a", "text": "take B", "by": "web"});
  let reply = answer(&mut engine, &request);
  assert_eq!(reply["ok"], true);
  assert_eq!(reply["delivered"], "uncertain");
  assert!(
    reply["note"]
      .as_str()
      .is_some_and(|note| note.contains("prompt uncertain"))
  );
  assert_eq!(prompts(tmp.path()).len(), 1);
  assert!(crate::attention::has_live(&engine.world, "a"));
  let journaled: Vec<(String, String)> = rows(&engine)
    .into_iter()
    .filter(|row| row.name == "answer-uncertain")
    .map(|row| (row.key, row.note))
    .collect();
  assert_eq!(
    journaled,
    [
      ("a".to_owned(), "take B".to_owned()),
      ("a".to_owned(), "by=web".to_owned())
    ]
  );
}

#[test]
fn herdr_timeout_and_stall_errors_are_uncertain_and_others_are_not() {
  let error = |code: &str| {
    let reply = json!({"error": {"code": code, "message": "agent busy"}}).to_string();
    crate::herdr_calls::delivered("agent.prompt", &reply).expect_err("herdr error")
  };
  assert!(super::uncertain(&error("timeout")));
  assert!(super::uncertain(&error("agent_prompt_stalled")));
  assert!(super::uncertain("read: timed out"));
  assert!(!super::uncertain(&error("agent_not_found")));
  assert!(!super::uncertain("prompt failed"));
}

#[test]
fn post_write_socket_failures_are_uncertain_and_pre_send_ones_are_not() {
  for err in [
    "read: connection reset by peer",
    "read: Connection reset by peer (os error 104)",
    "read: Connection reset by peer (os error 54)",
  ] {
    assert!(super::uncertain(err), "{err}");
  }
  for err in [
    "write: Broken pipe (os error 32)",
    "send: broken pipe",
    "connect: Connection reset by peer (os error 54)",
    "connect: Connection refused (os error 61)",
    "connect: No such file or directory (os error 2)",
    "no live agent for a",
  ] {
    assert!(!super::uncertain(err), "{err}");
  }
}
