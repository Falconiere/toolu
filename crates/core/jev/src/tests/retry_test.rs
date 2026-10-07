use toolu_http::Response;

use crate::retry::{delay, retryable};

/// Response headers, the attempt, and the expected pause units.
type Case<'a> = (&'a [(&'a str, &'a str)], u32, Option<u64>);

fn response(headers: &[(&str, &str)]) -> Response {
  Response {
    status: 429,
    headers: headers
      .iter()
      .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
      .collect(),
    body: Vec::new(),
  }
}

#[test]
fn only_timeouts_rate_limits_and_server_errors_are_retried() {
  for status in [408, 429, 500, 529] {
    assert!(retryable(status), "{status}");
  }
  for status in [400, 401, 403, 404, 302] {
    assert!(!retryable(status), "{status}");
  }
}

#[test]
fn the_delay_follows_jev_ts() {
  let cases: [Case<'_>; 10] = [
    (&[], 1, Some(1)),
    (&[], 2, Some(2)),
    (&[("retry-after", "5")], 1, Some(5)),
    (&[("retry-after", "1")], 2, Some(2)),
    (&[("retry-after", "61")], 1, None),
    (&[("retry-after", "100")], 1, None),
    (
      &[("retry-after", "soon"), ("retry-after-ms", "1500")],
      1,
      Some(2),
    ),
    (&[("retry-after-ms", "123456789")], 1, None),
    (&[("retry-after", "100"), ("retry-after-ms", "1")], 1, None),
    (
      &[("retry-after", "Wed, 21 Oct 2015 07:28:00 GMT")],
      1,
      Some(1),
    ),
  ];
  for (headers, attempt, expected) in cases {
    assert_eq!(
      delay(&response(headers), attempt),
      expected,
      "{headers:?} {attempt}"
    );
  }
}
