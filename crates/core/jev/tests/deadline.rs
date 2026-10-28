//! `ask_until`: one wall deadline bounds every attempt and pause, against the
//! loopback endpoint.

#[path = "helpers/endpoint.rs"]
mod endpoint;

use std::time::{Duration, Instant};

use endpoint::{Endpoint, PATH};
use serde_json::json;
use toolu_http_test_support::Reply;
use toolu_jev_client::{DEFAULT_MODEL, Error, Question, Questions, Reply as JevReply, State};

fn answer() -> String {
  json!({ "model": "jev-1.13.0", "answers": { "q": { "type": "noul", "noul": 0.9 } },
          "usage": { "input_tokens": 3, "output_tokens": 2, "cost": 0.0001 } })
  .to_string()
}

fn ask(endpoint: &Endpoint, deadline: Instant) -> Result<JevReply, Error> {
  let questions = Questions::single("q", Question::noul("Urgent?", None, None)?);
  endpoint
    .jev(&[])?
    .ask_until(&State::text("x"), DEFAULT_MODEL, &questions, deadline)
}

#[test]
fn a_far_deadline_retries_like_ask_and_keeps_the_cost() {
  let endpoint = Endpoint::start().expect("endpoint");
  endpoint
    .fixture
    .sequence(PATH, vec![Reply::new(503, "busy"), Reply::new(200, answer())])
    .expect("route");
  let reply = ask(&endpoint, Instant::now() + Duration::from_secs(30)).expect("reply");
  assert_eq!(reply.usage.cost_usd, Some(0.0001));
  assert_eq!(endpoint.bodies().len(), 2);
  assert_eq!(endpoint.last()["model"], DEFAULT_MODEL);
}

#[test]
fn a_slow_answer_times_out_at_the_deadline() {
  let endpoint = Endpoint::start().expect("endpoint");
  endpoint
    .fixture
    .route(
      PATH,
      Reply::new(200, answer()).delayed(Duration::from_secs(3)),
    )
    .expect("route");
  let started = Instant::now();
  let result = ask(&endpoint, started + Duration::from_millis(400));
  assert_eq!(result.map(|_| ()), Err(Error::Timeout));
  assert!(started.elapsed() < Duration::from_secs(2), "{:?}", started.elapsed());
}

#[test]
fn a_pause_past_the_deadline_returns_the_last_status_without_waiting() {
  let endpoint = Endpoint::start().expect("endpoint");
  endpoint
    .fixture
    .route(PATH, Reply::new(429, "busy").header("Retry-After", "50"))
    .expect("route");
  let started = Instant::now();
  let result = ask(&endpoint, started + Duration::from_millis(200));
  assert_eq!(
    result.map(|_| ()),
    Err(Error::Http {
      status: 429,
      body: "busy".into()
    })
  );
  assert_eq!(endpoint.bodies().len(), 1);
  assert!(started.elapsed() < Duration::from_millis(400));
}

#[test]
fn an_expired_deadline_sends_nothing() {
  let endpoint = Endpoint::start().expect("endpoint");
  endpoint
    .fixture
    .route(PATH, Reply::new(200, answer()))
    .expect("route");
  let result = ask(&endpoint, Instant::now());
  assert_eq!(result.map(|_| ()), Err(Error::Timeout));
  assert!(endpoint.bodies().is_empty());
}
