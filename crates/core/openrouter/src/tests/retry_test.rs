use std::time::{Duration, Instant};

use toolu_http::Response;

use super::{fits, retry_delay, retryable, status, status_delay};
use crate::Error;

#[test]
fn transient_statuses_and_retry_after_are_bounded() {
  for code in [408, 429, 500, 529] {
    assert!(retryable(code));
  }
  for code in [400, 401, 403, 404] {
    assert!(!retryable(code));
  }
  let response = Response {
    status: 429,
    headers: vec![("retry-after".into(), "5".into())],
    body: Vec::new(),
  };
  assert_eq!(
    retry_delay(&response, 1, Duration::from_millis(10)),
    Duration::from_millis(50)
  );
  assert_eq!(
    status(&response),
    Error::RateLimited {
      retry_after_secs: Some(5)
    }
  );
  assert!(!fits(
    Instant::now() + Duration::from_millis(5),
    Duration::from_millis(50)
  ));
}

#[test]
fn no_provider_is_typed_only_for_an_explicit_no_endpoint_error() {
  let body = br#"{"error":{"message":"No endpoints found that meet the provider constraints"}}"#;
  let response = Response {
    status: 404,
    headers: Vec::new(),
    body: body.to_vec(),
  };
  assert_eq!(status(&response), Error::NoProvider);
  let overloaded = Response {
    status: 503,
    ..response.clone()
  };
  assert_eq!(status(&overloaded), Error::NoProvider);
  assert_eq!(
    status_delay(&overloaded, 1, Duration::from_millis(10)),
    Err(Error::NoProvider)
  );
  let other = Response {
    body: br#"{"error":{"message":"model not found"}}"#.to_vec(),
    ..response
  };
  assert_eq!(status(&other), Error::Upstream(404));
}
