use super::{Fixture, Reply, Tracker, tracker};

fn open_issue(fixture: &Fixture) {
  fixture
    .route(
      "/repos/O/R/issues/2",
      Reply::new(200, r#"{"state":"open"}"#),
    )
    .expect("issue");
}

#[test]
fn a_failed_transition_retries_without_reposting_the_comment() {
  let fixture = Fixture::start().expect("fixture");
  let state = tempfile::tempdir().expect("state");
  open_issue(&fixture);
  fixture
    .sequence_when(
      "/repos/O/R/issues/2",
      "\"state\":\"closed\"",
      vec![Reply::new(503, "{}"), Reply::new(200, "{}")],
    )
    .expect("patch");
  fixture
    .route(
      "/repos/O/R/issues/2/comments?per_page=100",
      Reply::new(200, "[]"),
    )
    .expect("list");
  fixture
    .sequence_when(
      "/repos/O/R/issues/2/comments",
      "\"body\":",
      vec![Reply::new(201, r#"{"id":1}"#)],
    )
    .expect("post");
  assert!(
    tracker(&fixture)
      .close_issue("O/R#2", "Delivered", state.path())
      .is_err()
  );
  assert_eq!(
    tracker(&fixture).close_issue("O/R#2", "Delivered", state.path()),
    Ok("closed-manually".to_owned())
  );
  let posts = fixture
    .requests()
    .expect("requests")
    .iter()
    .filter(|request| request.method == "POST")
    .count();
  assert_eq!(posts, 1);
}

#[test]
fn a_lost_comment_response_is_reconciled_before_the_close() {
  let fixture = Fixture::start().expect("fixture");
  let state = tempfile::tempdir().expect("state");
  let other_state = tempfile::tempdir().expect("alternate state");
  open_issue(&fixture);
  fixture
    .sequence(
      "/repos/O/R/issues/2/comments?per_page=100",
      vec![
        Reply::new(200, "[]"),
        Reply::new(200, r#"[{"body":"Delivered"}]"#),
      ],
    )
    .expect("list");
  fixture
    .route("/repos/O/R/issues/2/comments", Reply::dropped_recorded())
    .expect("post");
  assert!(
    tracker(&fixture)
      .close_issue("O/R#2", "Delivered", state.path())
      .is_err()
  );
  assert_eq!(
    tracker(&fixture).close_issue("O/R#2", "Delivered", other_state.path()),
    Ok("closed-manually".to_owned())
  );
  let requests = fixture.requests().expect("requests");
  assert_eq!(
    requests.iter().filter(|row| row.method == "POST").count(),
    1
  );
  assert!(
    requests
      .iter()
      .any(|row| row.path.ends_with("comments?per_page=100"))
  );
}

#[test]
fn an_absent_comment_is_posted_after_a_failed_first_attempt() {
  let fixture = Fixture::start().expect("fixture");
  let state = tempfile::tempdir().expect("state");
  open_issue(&fixture);
  fixture
    .route(
      "/repos/O/R/issues/2/comments?per_page=100",
      Reply::new(200, "[]"),
    )
    .expect("list");
  fixture
    .sequence(
      "/repos/O/R/issues/2/comments",
      vec![Reply::new(503, "{}"), Reply::new(201, r#"{"id":1}"#)],
    )
    .expect("post");
  assert!(
    tracker(&fixture)
      .close_issue("O/R#2", "Delivered", state.path())
      .is_err()
  );
  assert!(
    tracker(&fixture)
      .close_issue("O/R#2", "Delivered", state.path())
      .is_ok()
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

#[test]
fn malformed_comment_list_fails_closed_and_an_already_closed_epic_stays_quiet() {
  let fixture = Fixture::start().expect("fixture");
  let state = tempfile::tempdir().expect("state");
  open_issue(&fixture);
  fixture
    .route("/repos/O/R/issues/2/comments", Reply::new(503, "{}"))
    .expect("post");
  fixture
    .sequence(
      "/repos/O/R/issues/2/comments?per_page=100",
      vec![Reply::new(200, "[]"), Reply::new(200, "{}")],
    )
    .expect("list");
  assert!(
    tracker(&fixture)
      .close_issue("O/R#2", "Delivered", state.path())
      .is_err()
  );
  assert!(
    tracker(&fixture)
      .close_issue("O/R#2", "Delivered", state.path())
      .is_err()
  );
  assert_eq!(
    fixture
      .requests()
      .expect("requests")
      .iter()
      .filter(|row| row.method == "POST")
      .count(),
    1
  );
  fixture
    .route(
      "/repos/O/R/issues/1",
      Reply::new(200, r#"{"state":"closed"}"#),
    )
    .expect("epic");
  assert_eq!(
    tracker(&fixture).close_epic("summary", state.path()),
    Ok("already-done".to_owned())
  );
}

#[test]
fn concurrent_closes_keep_one_comment_when_the_transition_keeps_failing() {
  let fixture = Fixture::start().expect("fixture");
  let state = tempfile::tempdir().expect("state");
  open_issue(&fixture);
  fixture
    .sequence_when(
      "/repos/O/R/issues/2",
      "\"state\":\"closed\"",
      vec![Reply::new(503, "{}")],
    )
    .expect("patch");
  fixture
    .route(
      "/repos/O/R/issues/2/comments",
      Reply::new(201, r#"{"id":1}"#),
    )
    .expect("post");
  fixture
    .route(
      "/repos/O/R/issues/2/comments?per_page=100",
      Reply::new(200, "[]"),
    )
    .expect("list");
  std::thread::scope(|scope| {
    let first = scope.spawn(|| tracker(&fixture).close_issue("O/R#2", "Delivered", state.path()));
    let second = scope.spawn(|| tracker(&fixture).close_issue("O/R#2", "Delivered", state.path()));
    assert!(first.join().expect("first").is_err());
    assert!(second.join().expect("second").is_err());
  });
  let requests = fixture.requests().expect("requests");
  assert_eq!(
    requests.iter().filter(|row| row.method == "POST").count(),
    1
  );
  assert_eq!(
    requests.iter().filter(|row| row.method == "PATCH").count(),
    2
  );
}
