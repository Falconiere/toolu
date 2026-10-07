use std::time::Duration;

use toolu_http::Response;
use toolu_runtime::env::Env;

use crate::Error;
use crate::policy::{Class, Retry, classify, one_shot, rate_wait};

fn response(status: u16, headers: &[(&str, &str)], body: &str) -> Response {
  Response {
    status,
    headers: headers
      .iter()
      .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
      .collect(),
    body: body.as_bytes().to_vec(),
  }
}

#[test]
fn one_shot_defaults_follow_gh_ts() {
  let (retry, timeout) = one_shot(&Env::default()).expect("policy");
  assert_eq!(retry.attempts, 3);
  assert_eq!(retry.backoff, [2, 4, 8].map(Duration::from_secs).to_vec());
  assert_eq!(retry.max_wait, Duration::from_secs(60));
  assert_eq!(timeout, Duration::from_secs(60));
  assert_eq!(retry.backoff(4), Duration::from_secs(2));
}

#[test]
fn one_shot_reads_and_checks_each_variable() {
  let env = Env::from_pairs([
    ("PB_GH_ATTEMPTS", "5"),
    ("PB_GH_BACKOFF", "0 0.5"),
    ("PB_GH_TIMEOUT", "1.5"),
  ]);
  let (retry, timeout) = one_shot(&env).expect("policy");
  assert_eq!(retry.attempts, 5);
  assert_eq!(retry.backoff, [Duration::ZERO, Duration::from_millis(500)]);
  assert_eq!(timeout, Duration::from_millis(1500));
  let cases = [
    (
      "PB_GH_ATTEMPTS",
      "0",
      "PB_GH_ATTEMPTS must be a positive integer",
    ),
    (
      "PB_GH_ATTEMPTS",
      "two",
      "PB_GH_ATTEMPTS must be a positive integer",
    ),
    (
      "PB_GH_BACKOFF",
      "1 x",
      "PB_GH_BACKOFF must list non-negative numbers of seconds",
    ),
    (
      "PB_GH_BACKOFF",
      "-1",
      "PB_GH_BACKOFF must list non-negative numbers of seconds",
    ),
    (
      "PB_GH_TIMEOUT",
      "0",
      "PB_GH_TIMEOUT must be a positive number of seconds",
    ),
    (
      "PB_GH_TIMEOUT",
      "inf",
      "PB_GH_TIMEOUT must be a positive number of seconds",
    ),
  ];
  for (name, value, message) in cases {
    assert_eq!(
      one_shot(&Env::from_pairs([(name, value)])).map(|_| ()),
      Err(Error::Config(message.to_owned())),
      "{name}={value}"
    );
  }
}

#[test]
fn the_scheduled_policy_makes_one_attempt_and_never_waits() {
  assert_eq!(
    Retry::scheduled(),
    Retry {
      attempts: 1,
      backoff: Vec::new(),
      max_wait: Duration::ZERO
    }
  );
}

#[test]
fn statuses_are_classed_as_gh_ts_does() {
  let cases = [
    (response(200, &[], ""), Class::Success),
    (response(304, &[], ""), Class::Success),
    (response(401, &[], ""), Class::Unauthorized),
    (response(404, &[], ""), Class::Permanent),
    (
      response(403, &[], "Resource not accessible"),
      Class::Permanent,
    ),
    (
      response(403, &[], "API rate limit exceeded"),
      Class::RateLimited { wait: None },
    ),
    (response(429, &[], ""), Class::RateLimited { wait: None }),
    (
      response(429, &[("Retry-After", "3")], ""),
      Class::RateLimited {
        wait: Some(Duration::from_secs(3)),
      },
    ),
    (response(502, &[], ""), Class::Transient),
  ];
  for (response, class) in cases {
    let status = response.status;
    assert_eq!(classify(&Ok(response)), class, "{status}");
  }
  assert_eq!(classify(&Err(toolu_http::Error::Timeout)), Class::Transient);
  assert_eq!(
    classify(&Err(toolu_http::Error::Transport("reset".into()))),
    Class::Transient
  );
  assert_eq!(
    classify(&Err(toolu_http::Error::BodyTooLarge)),
    Class::Permanent
  );
}

#[test]
fn the_wait_comes_from_retry_after_or_an_exhausted_limit() {
  let now = 1_000;
  let date = response(403, &[("Retry-After", "Wed, 21 Oct 2015 07:28:00 GMT")], "");
  assert_eq!(rate_wait(&date, now), None);
  let reset = response(
    403,
    &[
      ("X-RateLimit-Remaining", "0"),
      ("X-RateLimit-Reset", "4600"),
    ],
    "",
  );
  assert_eq!(rate_wait(&reset, now), Some(Duration::from_secs(3600)));
  let past = response(
    403,
    &[("X-RateLimit-Remaining", "0"), ("X-RateLimit-Reset", "10")],
    "",
  );
  assert_eq!(rate_wait(&past, now), Some(Duration::ZERO));
  let left = response(
    403,
    &[
      ("X-RateLimit-Remaining", "12"),
      ("X-RateLimit-Reset", "4600"),
    ],
    "",
  );
  assert_eq!(rate_wait(&left, now), None);
}
