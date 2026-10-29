use std::cell::Cell;

use toolu_http::{Auth, Client, Method, Request};
use toolu_http_test_support::{Fixture, Reply};

use crate::retry::{Budget, Class, Policy, github_failure, http_failure, with_retry};
use crate::support::http_client as client;

fn get(client: &Client, url: &str) -> Result<toolu_http::Response, toolu_http::Error> {
  client.send(&Request {
    method: Method::Get,
    url,
    auth: &Auth::None,
    headers: &[],
    body: None,
  })
}

#[test]
fn a_real_429_with_retry_after_waits_then_succeeds() {
  let fixture = Fixture::start().expect("fixture");
  fixture
    .sequence(
      "/graphql",
      vec![
        Reply::new(429, "slow down").header("Retry-After", "0"),
        Reply::new(200, r#"{"data":{"ok":true}}"#),
      ],
    )
    .expect("route");
  let http = client(&fixture);
  let url = fixture.url("/graphql");
  let policy = Policy {
    attempts: 3,
    base_ms: 1,
    max_sleep_ms: 5000,
  };
  let calls = Cell::new(0);
  let body = with_retry(&policy, &mut Budget::unbounded(), false, || {
    calls.set(calls.get() + 1);
    http_failure("Linear", &url, get(&http, &url))
  })
  .expect("body");
  assert_eq!(body, br#"{"data":{"ok":true}}"#);
  assert_eq!(calls.get(), 2);
}

#[test]
fn a_404_is_permanent_and_a_502_transient() {
  let fixture = Fixture::start().expect("fixture");
  fixture
    .route("/missing", Reply::new(404, "nope"))
    .expect("route");
  fixture
    .route("/flaky", Reply::new(502, "bad gateway"))
    .expect("route");
  let http = client(&fixture);
  let missing = fixture.url("/missing");
  let failure = http_failure("Jira", &missing, get(&http, &missing)).expect_err("404");
  assert_eq!(failure.class, Class::Permanent);
  assert_eq!(failure.message, format!("Jira: HTTP 404 {missing}: nope"));
  let flaky = fixture.url("/flaky");
  let failure = http_failure("Jira", &flaky, get(&http, &flaky)).expect_err("502");
  assert_eq!(failure.class, Class::Transient);
}

#[test]
fn the_status_decides_the_class_never_a_key_or_body_that_reads_like_one() {
  let fixture = Fixture::start().expect("fixture");
  let limited = r#"{"errors":[{"extensions":{"code":"RATELIMITED"}}]}"#;
  for (path, status, body, class) in [
    (
      "/rest/api/2/issue/PAY-503",
      404,
      "Issue PAY-503 does not exist",
      Class::Permanent,
    ),
    (
      "/rest/api/2/issue/PAY-429",
      401,
      "rate limit 429 too many requests",
      Class::Permanent,
    ),
    ("/graphql", 400, limited, Class::RateLimit),
  ] {
    fixture
      .route(path, Reply::new(status, body))
      .expect("route");
    let url = fixture.url(path);
    let failure = http_failure("Jira", &url, get(&client(&fixture), &url)).expect_err(path);
    assert_eq!(failure.class, class, "{path}");
  }
}

#[test]
fn a_graphql_ratelimited_error_in_a_200_is_a_rate_limit() {
  let fixture = Fixture::start().expect("fixture");
  let body = r#"{"errors":[{"message":"slow","extensions":{"code":"RATELIMITED"}}]}"#;
  fixture
    .route(
      "/graphql",
      Reply::new(200, body)
        .header("x-ratelimit-requests-reset", "1700000123456")
        .header("x-ratelimit-requests-remaining", "0"),
    )
    .expect("route");
  let http = client(&fixture);
  let url = fixture.url("/graphql");
  let failure = http_failure("Linear", &url, get(&http, &url)).expect_err("limited");
  assert_eq!(failure.class, Class::RateLimit);
  assert_eq!(failure.reset_at_ms, Some(1_700_000_123_456));
}

#[test]
fn github_errors_map_to_their_retry_class() {
  let limited = toolu_github::Error::RateLimited {
    status: 403,
    retry_after: None,
    rate: toolu_github::RateLimit {
      remaining: Some(0),
      reset: Some(1_700_000_100),
      ..toolu_github::RateLimit::default()
    },
    secondary: false,
  };
  let failure = github_failure(&limited);
  assert_eq!(failure.class, Class::RateLimit);
  assert_eq!(failure.reset_at_ms, Some(1_700_000_100_000));
  let busy = toolu_github::Error::Status {
    status: 503,
    message: String::new(),
  };
  assert_eq!(github_failure(&busy).class, Class::Transient);
  let gone = toolu_github::Error::Status {
    status: 404,
    message: "Not Found".into(),
  };
  assert_eq!(github_failure(&gone).class, Class::Permanent);
  assert_eq!(
    github_failure(&toolu_github::Error::Unauthorized).class,
    Class::Permanent
  );
}

#[test]
fn an_unsent_http_request_remains_retryable_for_reads() {
  let failure = http_failure(
    "Linear",
    "https://example.invalid/graphql",
    Err(toolu_http::Error::NotSent("connection refused".into())),
  )
  .expect_err("unsent request");
  assert_eq!(failure.class, Class::Transient);
}
