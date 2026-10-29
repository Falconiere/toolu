use super::{Fixture, Reply, Tracker, issue, json, route, tracker};

fn request_count(fixture: &Fixture, path: &str, method: &str) -> usize {
  fixture
    .requests()
    .expect("requests")
    .iter()
    .filter(|row| row.path == path && row.method == method)
    .count()
}

fn lost_comment_pages(fixture: &Fixture) {
  fixture
    .sequence(
      "/rest/api/3/issue/PAY-7/comment?startAt=0&maxResults=100",
      vec![
        Reply::new(
          200,
          json!({"startAt": 0, "total": 0, "comments": []}).to_string(),
        ),
        Reply::new(
          200,
          json!({"startAt": 0, "total": 2, "comments": [{"body": "other"}]}).to_string(),
        ),
      ],
    )
    .expect("list");
  route(
    fixture,
    "/rest/api/3/issue/PAY-7/comment?startAt=1&maxResults=100",
    &json!({"startAt": 1, "total": 2, "comments": [{"body": {"type": "doc", "content": [
      {"type": "paragraph", "content": [{"type": "text", "text": "Delivered"}]}
    ]}}]}),
  );
}

#[test]
fn issue_and_epic_retry_only_the_failed_transition() {
  for (key, epic) in [("PAY-2", false), ("PAY-7", true)] {
    let fixture = Fixture::start().expect("fixture");
    let state = tempfile::tempdir().expect("state");
    let issue_path = format!("/rest/api/3/issue/{key}?fields=status");
    let comment_path = format!("/rest/api/3/issue/{key}/comment");
    let transition_path = format!("/rest/api/3/issue/{key}/transitions");
    route(&fixture, &issue_path, &issue(key, false));
    route(&fixture, &comment_path, &json!({}));
    route(
      &fixture,
      &format!("{comment_path}?startAt=0&maxResults=100"),
      &json!({"startAt": 0, "total": 0, "comments": []}),
    );
    route(
      &fixture,
      &transition_path,
      &json!({"transitions": [{"id": "31", "name": "Done", "to": {"statusCategory": {"key": "done"}}}]}),
    );
    fixture
      .sequence_when(
        &transition_path,
        "\"transition\":",
        vec![Reply::new(503, "{}"), Reply::new(200, "{}")],
      )
      .expect("write");
    let run = || {
      let jira = tracker(&fixture);
      if epic {
        jira.close_epic("Delivered", state.path())
      } else {
        jira.close_issue(key, "Delivered", state.path())
      }
    };
    assert!(run().is_err());
    assert_eq!(run(), Ok("transitioned-done".to_owned()));
    assert_eq!(request_count(&fixture, &comment_path, "POST"), 1);
    assert_eq!(request_count(&fixture, &transition_path, "POST"), 2);
  }
}

#[test]
fn a_lost_comment_response_is_found_on_a_later_adf_page() {
  let fixture = Fixture::start().expect("fixture");
  let state = tempfile::tempdir().expect("state");
  route(
    &fixture,
    "/rest/api/3/issue/PAY-7?fields=status",
    &issue("PAY-7", false),
  );
  fixture
    .route("/rest/api/3/issue/PAY-7/comment", Reply::dropped_recorded())
    .expect("post");
  lost_comment_pages(&fixture);
  route(
    &fixture,
    "/rest/api/3/issue/PAY-7/transitions",
    &json!({"transitions": []}),
  );
  assert!(
    tracker(&fixture)
      .close_epic("Delivered", state.path())
      .is_err()
  );
  assert_eq!(
    tracker(&fixture).close_epic("changed", state.path()),
    Ok("no-done-transition".to_owned())
  );
  assert_eq!(
    request_count(&fixture, "/rest/api/3/issue/PAY-7/comment", "POST"),
    1
  );
  let requests = fixture.requests().expect("requests");
  assert!(
    requests
      .iter()
      .any(|row| row.path.ends_with("comment?startAt=1&maxResults=100"))
  );
}

#[test]
fn a_done_epic_gets_no_comment() {
  let fixture = Fixture::start().expect("fixture");
  let state = tempfile::tempdir().expect("state");
  route(
    &fixture,
    "/rest/api/3/issue/PAY-7?fields=status",
    &issue("PAY-7", true),
  );
  assert_eq!(
    tracker(&fixture).close_epic("summary", state.path()),
    Ok("already-done".to_owned())
  );
  assert_eq!(fixture.requests().expect("requests").len(), 1);
}

#[test]
fn a_missing_jira_comment_is_posted_on_retry() {
  let fixture = Fixture::start().expect("fixture");
  let state = tempfile::tempdir().expect("state");
  route(
    &fixture,
    "/rest/api/3/issue/PAY-7?fields=status",
    &issue("PAY-7", false),
  );
  route(
    &fixture,
    "/rest/api/3/issue/PAY-7/comment?startAt=0&maxResults=100",
    &json!({"startAt": 0, "total": 0, "comments": []}),
  );
  fixture
    .sequence(
      "/rest/api/3/issue/PAY-7/comment",
      vec![Reply::new(503, "{}"), Reply::new(201, "{}")],
    )
    .expect("post");
  route(
    &fixture,
    "/rest/api/3/issue/PAY-7/transitions",
    &json!({"transitions": []}),
  );
  assert!(
    tracker(&fixture)
      .close_epic("Delivered", state.path())
      .is_err()
  );
  assert_eq!(
    tracker(&fixture).close_epic("Delivered", state.path()),
    Ok("no-done-transition".to_owned())
  );
  assert_eq!(
    fixture
      .requests()
      .expect("requests")
      .iter()
      .filter(|row| row.method == "POST")
      .count(),
    2
  );
}
