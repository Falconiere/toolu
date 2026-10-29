use super::{Fixture, Reply, Tracker, json, replies, sent, tracker};
use serde_json::Value;

fn started_issue() -> Value {
  json!({"issue": {"id": "uuid-5", "state": {"type": "started"},
    "team": {"states": {"nodes": [{"id": "st-done", "position": 1}]}}}})
}

fn answer(data: &Value) -> Reply {
  Reply::new(200, json!({"data": data}).to_string())
}

fn serve(fixture: &Fixture, operation: &str, answers: Vec<Reply>) {
  fixture
    .sequence_when("/graphql", operation, answers)
    .expect("route");
}

fn comments(nodes: &Value, more: bool, cursor: &Value) -> Reply {
  answer(&json!({"issue": {"comments": {
    "nodes": nodes, "pageInfo": {"hasNextPage": more, "endCursor": cursor}
  }}}))
}

fn mutation_count(fixture: &Fixture, operation: &str) -> usize {
  sent(fixture)
    .iter()
    .filter(|body| {
      body["query"]
        .as_str()
        .is_some_and(|query| query.contains(operation))
    })
    .count()
}

#[test]
fn issue_and_epic_retry_only_a_failed_state_update() {
  for epic in [false, true] {
    let fixture = Fixture::start().expect("fixture");
    let state = tempfile::tempdir().expect("state");
    replies(&fixture, &[started_issue()]);
    serve(
      &fixture,
      "commentCreate",
      vec![answer(&json!({"commentCreate": {"success": true}}))],
    );
    serve(
      &fixture,
      "comments(first:100",
      vec![comments(&json!([]), false, &Value::Null)],
    );
    serve(
      &fixture,
      "issueUpdate",
      vec![
        Reply::new(503, "{}"),
        answer(&json!({"issueUpdate": {"success": true}})),
      ],
    );
    let run = || {
      let linear = tracker(&fixture, "linear:ENG-1", "k");
      if epic {
        linear.close_epic("Delivered", state.path())
      } else {
        linear.close_issue("ENG-5", "Delivered", state.path())
      }
    };
    assert!(run().is_err());
    assert_eq!(run(), Ok("completed".to_owned()));
    assert_eq!(mutation_count(&fixture, "commentCreate"), 1);
  }
}

#[test]
fn a_lost_comment_response_is_found_before_retrying_the_update() {
  let fixture = Fixture::start().expect("fixture");
  let state = tempfile::tempdir().expect("state");
  replies(&fixture, &[started_issue()]);
  serve(&fixture, "commentCreate", vec![Reply::dropped_recorded()]);
  serve(
    &fixture,
    "comments(first:100",
    vec![
      comments(&json!([]), false, &Value::Null),
      comments(&json!([]), true, &json!("page-2")),
      comments(&json!([{"body": "Delivered"}]), false, &Value::Null),
    ],
  );
  serve(
    &fixture,
    "issueUpdate",
    vec![answer(&json!({"issueUpdate": {"success": true}}))],
  );
  assert!(
    tracker(&fixture, "linear:ENG-1", "k")
      .close_issue("ENG-5", "Delivered", state.path())
      .is_err()
  );
  assert_eq!(
    tracker(&fixture, "linear:ENG-1", "k").close_issue("ENG-5", "changed", state.path()),
    Ok("completed".to_owned())
  );
  let bodies = sent(&fixture);
  assert_eq!(mutation_count(&fixture, "commentCreate"), 1);
  assert!(bodies.iter().any(|body| {
    body["query"]
      .as_str()
      .is_some_and(|query| query.contains("comments(first:100"))
  }));
  assert!(
    bodies
      .iter()
      .any(|body| body["variables"]["after"] == "page-2")
  );
}

#[test]
fn a_refused_comment_is_reposted_after_the_remote_list_is_empty() {
  let fixture = Fixture::start().expect("fixture");
  let state = tempfile::tempdir().expect("state");
  replies(&fixture, &[started_issue()]);
  serve(
    &fixture,
    "commentCreate",
    vec![
      answer(&json!({"commentCreate": {"success": false}})),
      answer(&json!({"commentCreate": {"success": true}})),
    ],
  );
  serve(
    &fixture,
    "comments(first:100",
    vec![comments(&json!([]), false, &Value::Null)],
  );
  serve(
    &fixture,
    "issueUpdate",
    vec![answer(&json!({"issueUpdate": {"success": true}}))],
  );
  let first =
    tracker(&fixture, "linear:ENG-1", "k").close_issue("ENG-5", "Delivered", state.path());
  assert_eq!(first, Err("Linear: commentCreate failed".to_owned()));
  assert_eq!(
    tracker(&fixture, "linear:ENG-1", "k").close_issue("ENG-5", "Delivered", state.path()),
    Ok("completed".to_owned())
  );
  assert_eq!(mutation_count(&fixture, "commentCreate"), 2);
}

#[test]
fn a_canceled_epic_gets_no_comment() {
  let fixture = Fixture::start().expect("fixture");
  let state = tempfile::tempdir().expect("state");
  replies(
    &fixture,
    &[json!({"issue": {"id": "uuid-5", "state": {"type": "canceled"}}})],
  );
  assert_eq!(
    tracker(&fixture, "linear:ENG-1", "k").close_epic("summary", state.path()),
    Ok("already-done".to_owned())
  );
  assert_eq!(sent(&fixture).len(), 1);
}
