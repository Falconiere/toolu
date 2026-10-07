//! `jev.ts`'s retries against the loopback endpoint: transient statuses,
//! `retry-after`, dropped connections, a zero timeout and the body cap; and the
//! key never printed.

#[path = "helpers/endpoint.rs"]
mod endpoint;

use std::time::{Duration, Instant};

use endpoint::{Endpoint, PATH};
use serde_json::json;
use toolu_http_test_support::Reply;
use toolu_jev_client::{DEFAULT_MODEL, Error, Question, Questions, State};

fn answer() -> String {
  json!({ "model": "jev-1.13.0", "answers": { "q": { "type": "noul", "noul": 0.92 } },
          "usage": { "input_tokens": 3, "output_tokens": 2 } })
  .to_string()
}

fn noul() -> Result<Questions, Error> {
  Ok(Questions::single(
    "q",
    Question::noul("Urgent?", None, None)?,
  ))
}

#[test]
fn a_408_and_a_529_are_retried_to_an_answer() {
  let endpoint = Endpoint::start().expect("endpoint");
  endpoint
    .fixture
    .sequence(
      PATH,
      vec![
        Reply::new(408, "transient"),
        Reply::new(529, "overloaded"),
        Reply::new(200, answer()),
      ],
    )
    .expect("route");
  assert!(
    endpoint
      .jev(&[])
      .expect("jev")
      .ask(&State::text("x"), DEFAULT_MODEL, &noul().expect("noul"))
      .is_ok()
  );
  assert_eq!(endpoint.last()["questions"]["q"]["instructions"], "Urgent?");
  assert_eq!(endpoint.bodies().len(), 3);
}

#[test]
fn a_retry_after_over_60_returns_the_status_without_waiting() {
  let endpoint = Endpoint::start().expect("endpoint");
  endpoint
    .fixture
    .route(PATH, Reply::new(429, "busy").header("Retry-After", "61"))
    .expect("route");
  let result =
    endpoint
      .jev(&[])
      .expect("jev")
      .ask(&State::text("x"), DEFAULT_MODEL, &noul().expect("noul"));
  assert_eq!(
    result,
    Err(Error::Http {
      status: 429,
      body: "busy".into()
    })
  );
  assert_eq!(endpoint.bodies().len(), 1);
}

#[test]
fn retry_after_and_retry_after_ms_count_pause_units() {
  let endpoint = Endpoint::start().expect("endpoint");
  endpoint
    .fixture
    .sequence(
      PATH,
      vec![
        Reply::new(503, "").header("Retry-After-Ms", "29001"),
        Reply::new(503, "").header("Retry-After", "30"),
        Reply::new(200, answer()),
      ],
    )
    .expect("route");
  let started = Instant::now();
  assert!(
    endpoint
      .jev(&[])
      .expect("jev")
      .ask(&State::text("x"), DEFAULT_MODEL, &noul().expect("noul"))
      .is_ok()
  );
  // 30 units each (29001 ms rounds up), of 10 ms: unreachable by the base pause.
  assert!(started.elapsed() >= Duration::from_millis(600));
  assert_eq!(endpoint.bodies().len(), 3);
}

#[test]
fn a_dropped_connection_is_retried_to_an_answer() {
  let endpoint = Endpoint::start().expect("endpoint");
  endpoint
    .fixture
    .sequence(PATH, vec![Reply::dropped(), Reply::new(200, answer())])
    .expect("route");
  assert!(
    endpoint
      .jev(&[])
      .expect("jev")
      .ask(&State::text("x"), DEFAULT_MODEL, &noul().expect("noul"))
      .is_ok()
  );
  assert!(endpoint.fixture.connects().expect("connects").len() >= 2);
  assert_eq!(endpoint.bodies().len(), 1);
}

#[test]
fn a_zero_timeout_times_out_every_attempt_unsent() {
  let endpoint = Endpoint::start().expect("endpoint");
  endpoint
    .fixture
    .route(PATH, Reply::new(200, answer()))
    .expect("route");
  let jev = endpoint.jev(&[("JEV_TIMEOUT", "0")]).expect("jev");
  let started = Instant::now();
  assert_eq!(
    jev.ask(&State::text("x"), DEFAULT_MODEL, &noul().expect("noul")),
    Err(Error::Timeout)
  );
  // Three attempts: pauses of 1 and 2 units of 10 ms between them.
  assert!(started.elapsed() >= Duration::from_millis(30));
  assert_eq!(endpoint.fixture.connects().expect("connects").len(), 0);
}

#[test]
fn a_body_over_the_cap_is_not_retried() {
  let endpoint = Endpoint::start().expect("endpoint");
  endpoint
    .fixture
    .route(PATH, Reply::new(200, vec![b' '; 1024 * 1024 + 1]))
    .expect("route");
  let result =
    endpoint
      .jev(&[])
      .expect("jev")
      .ask(&State::text("x"), DEFAULT_MODEL, &noul().expect("noul"));
  assert_eq!(
    result,
    Err(Error::Transport(toolu_http::Error::BodyTooLarge))
  );
  assert_eq!(endpoint.bodies().len(), 1);
}

#[test]
fn the_key_is_in_no_error_or_debug_output() {
  let endpoint = Endpoint::start().expect("endpoint");
  let key = format!("jev-sentinel-{}", std::process::id());
  endpoint
    .fixture
    .sequence(
      PATH,
      vec![
        Reply::new(401, format!("bad key {key}")),
        Reply::new(200, "not json"),
        Reply::dropped(),
      ],
    )
    .expect("route");
  let jev = endpoint.jev(&[("TYPESAFE_API_KEY", &key)]).expect("jev");
  let mut shown = vec![format!("{jev:?}")];
  for _ in 0..3 {
    let result = jev.ask(&State::text("x"), DEFAULT_MODEL, &noul().expect("noul"));
    shown.push(format!("{result:?}"));
    if let Err(err) = result {
      shown.push(err.to_string());
    }
  }
  let shown = shown.join("\n");
  let requests = endpoint.fixture.requests().expect("requests");
  assert!(
    requests[0]
      .headers
      .get("authorization")
      .is_some_and(|value| value.ends_with(&key))
  );
  assert!(!shown.contains(&key), "{shown}");
}

#[test]
fn a_status_that_stays_retryable_ends_after_three_attempts() {
  let endpoint = Endpoint::start().expect("endpoint");
  endpoint
    .fixture
    .route(PATH, Reply::new(503, "down"))
    .expect("route");
  let result =
    endpoint
      .jev(&[])
      .expect("jev")
      .ask(&State::text("x"), DEFAULT_MODEL, &noul().expect("noul"));
  assert_eq!(
    result,
    Err(Error::Http {
      status: 503,
      body: "down".into()
    })
  );
  assert_eq!(endpoint.bodies().len(), 3);
}

#[test]
fn a_redirect_is_returned_not_followed() {
  let endpoint = Endpoint::start().expect("endpoint");
  let elsewhere = endpoint.fixture.second_url(PATH);
  endpoint
    .fixture
    .route(
      PATH,
      Reply::new(302, "moved").header("Location", &elsewhere),
    )
    .expect("route");
  let result =
    endpoint
      .jev(&[])
      .expect("jev")
      .ask(&State::text("x"), DEFAULT_MODEL, &noul().expect("noul"));
  assert_eq!(
    result,
    Err(Error::Http {
      status: 302,
      body: "moved".into()
    })
  );
  assert_eq!(endpoint.bodies().len(), 1);
}
