use std::time::Duration;

use serde_json::Value;
use toolu_http_test_support::{Fixture, Reply};

use super::tests::{env, notice, outcome, post_once, settings};
use super::{Notifier, Paths};

#[test]
fn notify_sends_once_when_the_receiver_acknowledges_immediately() {
  let tmp = tempfile::tempdir().expect("temp");
  let fixture = Fixture::start().expect("fixture");
  fixture
    .route("/hook", Reply::new(200, "ok"))
    .expect("route");
  let paths = post_once(tmp.path(), &fixture);
  let row = outcome(&paths, &["notify", "notify-failed"]);
  assert_eq!(row.note, "channel=url attempts=1 status=200");
  assert_eq!(fixture.requests().expect("requests").len(), 1);
}

#[test]
fn notify_repeats_a_committed_post_when_its_response_is_lost() {
  let tmp = tempfile::tempdir().expect("temp");
  let fixture = Fixture::start().expect("fixture");
  fixture
    .sequence(
      "/hook",
      vec![Reply::dropped_recorded(), Reply::new(200, "ok")],
    )
    .expect("sequence");
  let paths = post_once(tmp.path(), &fixture);
  let row = outcome(&paths, &["notify", "notify-failed"]);
  assert_eq!(row.note, "channel=url attempts=2 status=200");
  let requests = fixture.requests().expect("requests");
  assert_eq!(requests.len(), 2);
  assert_eq!(requests[0].method, "POST");
  assert_eq!(requests[0].body, requests[1].body);
  let payload: Value = serde_json::from_slice(&requests[0].body).expect("json");
  assert!(payload.get("delivery_id").is_none());
  assert_eq!(requests[0].headers["content-type"], "application/json");
}

#[test]
fn notify_retries_after_a_post_commit_response_timeout() {
  let tmp = tempfile::tempdir().expect("temp");
  let fixture = Fixture::start().expect("fixture");
  fixture
    .sequence(
      "/hook",
      vec![
        Reply::new(200, "late").delayed(Duration::from_secs(1)),
        Reply::new(200, "ok"),
      ],
    )
    .expect("sequence");
  let paths = Paths::at(tmp.path());
  let url = fixture.url("/hook");
  let mut configured = settings(env(tmp.path(), Some(&url)), &fixture);
  configured.timeout = Duration::from_millis(200);
  let notifier = Notifier::spawn(configured, paths.clone());
  notifier.tx.try_send(notice("which?")).expect("send");
  let row = outcome(&paths, &["notify", "notify-failed"]);
  assert_eq!(row.note, "channel=url attempts=2 status=200");
  let requests = fixture.requests().expect("requests");
  assert_eq!(requests.len(), 2);
  assert_eq!(requests[0].body, requests[1].body);
}
