//! Comment-creating REST writes are observed on a real loopback origin.

#[path = "helpers/api.rs"]
mod api;

use std::time::{Duration, Instant};

use api::Api;
use serde_json::json;
use toolu_github::{Client, Config, Error, RestRequest};
use toolu_http::Method;
use toolu_http_test_support::Reply;
use toolu_runtime::env::Env;

fn one_shot(api: &Api) -> Result<Client, Error> {
  let env = Env::from_pairs([("GH_TOKEN", "retry-token"), ("PB_GH_BACKOFF", "0 0 0")]);
  api.client(Config::one_shot(&env)?, &env)
}

fn post(client: &Client) -> Result<toolu_github::Reply<toolu_github::Rest>, Error> {
  let body = json!({"body":"reply"});
  client.rest(&RestRequest {
    method: Method::Post,
    path: "/comments",
    body: Some(&body),
    etag: None,
  })
}

#[test]
fn a_server_error_does_not_retry_comment_creation() {
  let api = Api::start().expect("api");
  api
    .fixture
    .sequence(
      "/comments",
      vec![Reply::new(502, "{}"), Reply::new(201, r#"{"id":1}"#)],
    )
    .expect("route");
  assert!(matches!(
    post(&one_shot(&api).expect("client")),
    Err(Error::Status { status: 502, .. })
  ));
  assert_eq!(api.fixture.requests().expect("requests").len(), 1);
}

#[test]
fn a_committed_comment_with_a_lost_response_is_sent_once() {
  let api = Api::start().expect("api");
  api
    .fixture
    .sequence(
      "/comments",
      vec![
        Reply::committed_then_dropped(),
        Reply::new(201, r#"{"id":1}"#),
      ],
    )
    .expect("route");
  assert!(matches!(
    post(&one_shot(&api).expect("client")),
    Err(Error::Transport(toolu_http::Error::Transport(_)))
  ));
  assert_eq!(api.fixture.requests().expect("requests").len(), 1);
  assert_eq!(api.fixture.connects().expect("connects").len(), 1);
}

#[test]
fn a_rate_limited_comment_post_does_not_wait_or_retry() {
  let api = Api::start().expect("api");
  api
    .fixture
    .route(
      "/comments",
      Reply::new(429, "{}").header("Retry-After", "1"),
    )
    .expect("route");
  let started = Instant::now();
  assert!(matches!(
    post(&one_shot(&api).expect("client")),
    Err(Error::RateLimited { status: 429, .. })
  ));
  assert!(started.elapsed() < Duration::from_secs(1));
  assert_eq!(api.fixture.requests().expect("requests").len(), 1);
}
