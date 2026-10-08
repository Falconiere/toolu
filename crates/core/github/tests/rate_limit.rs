//! Rate-limit metadata reaches scheduled callers without a hidden retry.

#[path = "helpers/api.rs"]
mod api;

use std::time::Duration;

use api::Api;
use toolu_github::{Config, Error, RateLimit};
use toolu_http_test_support::Reply;
use toolu_runtime::env::Env;

#[test]
fn rate_limit_error_keeps_primary_headers_and_secondary_throttle() {
  let api = Api::start().expect("api");
  api
    .fixture
    .route(
      "/secondary",
      Reply::new(429, r#"{"message":"secondary rate limit"}"#)
        .header("Retry-After", "60")
        .header("X-RateLimit-Resource", "core")
        .header("X-RateLimit-Remaining", "4900")
        .header("X-RateLimit-Used", "100"),
    )
    .expect("route");
  let client = api
    .client(
      Config::scheduled(),
      &Env::from_pairs([("GH_TOKEN", "rate-token")]),
    )
    .expect("client");
  let result = client.get("/secondary", None);
  assert!(matches!(
    result,
    Err(Error::RateLimited {
      status: 429,
      retry_after: Some(wait),
      rate,
      secondary: true,
    }) if wait == Duration::from_secs(60)
      && rate.resource.as_deref() == Some("core")
      && rate.remaining == Some(4900)
      && rate.used == Some(100)
  ));
  assert_eq!(api.fixture.requests().expect("requests").len(), 1);
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
  let client = api
    .client(
      Config::scheduled(),
      &Env::from_pairs([("GH_TOKEN", "rate-token")]),
    )
    .expect("client");
  assert_eq!(
    client.get("/x", None).map(|_| ()),
    Err(Error::RateLimited {
      status: 429,
      retry_after: Some(Duration::from_secs(1)),
      rate: RateLimit::default(),
      secondary: false,
    })
  );
  assert_eq!(api.fixture.requests().expect("requests").len(), 1);
}
