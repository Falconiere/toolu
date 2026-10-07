//! The one-shot policy keeps pr-babysit's gh retries (`gh.ts`); the scheduled
//! policy makes one attempt and never sleeps.

#[path = "helpers/api.rs"]
mod api;

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use api::Api;
use toolu_github::{Client, Config, Error, Rest};
use toolu_http_test_support::Reply;
use toolu_runtime::env::Env;

fn env() -> Env {
  Env::from_pairs([("GH_TOKEN", "retry-token"), ("PB_GH_BACKOFF", "0 0 0")])
}

fn one_shot(api: &Api) -> Result<Client, Error> {
  api.client(Config::one_shot(&env())?, &env())
}

fn requests(api: &Api) -> usize {
  api.fixture.requests().map_or(0, |requests| requests.len())
}

#[test]
fn a_server_error_is_retried_until_it_succeeds() {
  let api = Api::start().expect("api");
  api
    .fixture
    .sequence("/x", vec![Reply::new(502, ""), Reply::new(200, "{}")])
    .expect("route");
  let reply = one_shot(&api)
    .expect("client")
    .get("/x", None)
    .expect("reply");
  assert!(matches!(reply.data, Rest::Fresh(_)));
  assert_eq!(reply.attempts, 2);
}

#[test]
fn exhausted_attempts_return_the_last_status() {
  let api = Api::start().expect("api");
  api
    .fixture
    .route("/x", Reply::new(502, r#"{"message":"Bad Gateway"}"#))
    .expect("route");
  assert_eq!(
    one_shot(&api).expect("client").get("/x", None).map(|_| ()),
    Err(Error::Status {
      status: 502,
      message: "Bad Gateway".into()
    })
  );
  assert_eq!(requests(&api), 3);
}

#[test]
fn a_not_found_and_a_plain_forbidden_are_permanent() {
  let api = Api::start().expect("api");
  api
    .fixture
    .route("/missing", Reply::new(404, r#"{"message":"Not Found"}"#))
    .expect("route");
  api
    .fixture
    .route(
      "/forbidden",
      Reply::new(403, r#"{"message":"Resource not accessible"}"#),
    )
    .expect("route");
  let client = one_shot(&api).expect("client");
  assert!(matches!(
    client.get("/missing", None),
    Err(Error::Status { status: 404, .. })
  ));
  assert!(matches!(
    client.get("/forbidden", None),
    Err(Error::Status { status: 403, .. })
  ));
  assert_eq!(requests(&api), 2);
}

#[test]
fn a_rate_limit_message_is_retried() {
  let api = Api::start().expect("api");
  api
    .fixture
    .sequence(
      "/x",
      vec![
        Reply::new(403, r#"{"message":"API rate limit exceeded for user"}"#),
        Reply::new(200, "{}"),
      ],
    )
    .expect("route");
  assert_eq!(
    one_shot(&api)
      .expect("client")
      .get("/x", None)
      .expect("reply")
      .attempts,
    2
  );
}

#[test]
fn a_short_retry_after_is_waited_for() {
  let api = Api::start().expect("api");
  api
    .fixture
    .sequence(
      "/x",
      vec![
        Reply::new(403, "{}").header("Retry-After", "1"),
        Reply::new(200, "{}"),
      ],
    )
    .expect("route");
  let started = Instant::now();
  assert_eq!(
    one_shot(&api)
      .expect("client")
      .get("/x", None)
      .expect("reply")
      .attempts,
    2
  );
  assert!(started.elapsed() >= Duration::from_secs(1));
  assert_eq!(requests(&api), 2);
}

#[test]
fn a_long_wait_is_returned_at_once() {
  let api = Api::start().expect("api");
  api
    .fixture
    .route(
      "/retry-after",
      Reply::new(403, "{}").header("Retry-After", "120"),
    )
    .expect("route");
  let reset = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .expect("clock")
    .as_secs()
    + 3600;
  api
    .fixture
    .route(
      "/exhausted",
      Reply::new(403, "{}")
        .header("X-RateLimit-Remaining", "0")
        .header("X-RateLimit-Reset", &reset.to_string()),
    )
    .expect("route");
  let client = one_shot(&api).expect("client");
  assert_eq!(
    client.get("/retry-after", None).map(|_| ()),
    Err(Error::RateLimited {
      status: 403,
      retry_after: Some(Duration::from_secs(120))
    })
  );
  let Err(Error::RateLimited {
    retry_after: Some(wait),
    ..
  }) = client.get("/exhausted", None)
  else {
    panic!("expected a rate limit");
  };
  assert!((3590..=3600).contains(&wait.as_secs()), "{wait:?}");
  assert_eq!(requests(&api), 2);
}

#[test]
fn the_scheduled_policy_never_sleeps() {
  let api = Api::start().expect("api");
  api
    .fixture
    .sequence(
      "/x",
      vec![
        Reply::new(429, "{}").header("Retry-After", "1"),
        Reply::new(200, "{}"),
      ],
    )
    .expect("route");
  let client = api.client(Config::scheduled(), &env()).expect("client");
  assert_eq!(
    client.get("/x", None).map(|_| ()),
    Err(Error::RateLimited {
      status: 429,
      retry_after: Some(Duration::from_secs(1))
    })
  );
  assert_eq!(requests(&api), 1);
}

#[test]
fn an_invalid_variable_is_named() {
  for (name, value) in [
    ("PB_GH_ATTEMPTS", "0"),
    ("PB_GH_BACKOFF", "x"),
    ("PB_GH_TIMEOUT", "0"),
  ] {
    let Err(Error::Config(message)) = Config::one_shot(&Env::from_pairs([(name, value)])) else {
      panic!("{name}={value} was accepted");
    };
    assert!(message.starts_with(name), "{message}");
  }
}
