use std::time::Duration;

use serde_json::json;
use toolu_github::Error;
use toolu_github::RateLimit;
use toolu_http_test_support::{Fixture, Reply};

use super::{Reader, failure, link_next};
use crate::recorded;

fn link(value: &str) -> Vec<(String, String)> {
  vec![
    ("content-type".into(), "application/json".into()),
    ("link".into(), value.into()),
  ]
}

#[test]
fn link_next_returns_the_next_pages_path_and_query() {
  let header = "<https://api.github.com/repositories/1/issues/2/comments?per_page=2&page=3>; \
                rel=\"next\", <https://api.github.com/repositories/1/issues/2/comments?page=9>; \
                rel=\"last\"";
  assert_eq!(
    link_next(&link(header)).as_deref(),
    Some("/repositories/1/issues/2/comments?per_page=2&page=3")
  );
  let last_page = "<https://api.github.com/x?page=1>; rel=\"prev\"";
  assert_eq!(link_next(&link(last_page)), None);
  assert_eq!(link_next(&[]), None);
  assert_eq!(
    link_next(&link("<https://api.github.com>; rel=\"next\"")),
    None
  );
}

#[test]
fn rest_pages_follow_links_until_the_last_page() {
  let fixture = Fixture::start().unwrap();
  let pages = [json!([1, 2]), json!([3, 4]), json!([5])];
  recorded::rest_pages(&fixture, "/list?per_page=2", &pages);
  let client = recorded::client(&fixture);
  let read = Reader::new(&client)
    .rest_pages("list", "/list?per_page=2")
    .unwrap();
  assert_eq!(read, pages);
  let paths: Vec<String> = fixture
    .requests()
    .unwrap()
    .into_iter()
    .map(|r| r.path)
    .collect();
  assert_eq!(
    paths,
    [
      "/list?per_page=2",
      "/list?per_page=2&page=2",
      "/list?per_page=2&page=3"
    ]
  );
}

#[test]
fn a_reply_that_is_not_json_is_invalid_json() {
  let fixture = Fixture::start().unwrap();
  fixture.route("/list", Reply::new(200, "<html>")).unwrap();
  let client = recorded::client(&fixture);
  let failure = Reader::new(&client)
    .rest_pages("comments", "/list")
    .err()
    .unwrap();
  assert_eq!(failure.code, "invalid_json");
  assert_eq!(failure.extra["source"], "comments");
}

#[test]
fn a_head_without_a_sha_is_invalid_json() {
  let fixture = Fixture::start().unwrap();
  fixture
    .route(
      "/repos/o/r/pulls/1",
      Reply::new(200, r#"{"head":{"sha":""}}"#),
    )
    .unwrap();
  let client = recorded::client(&fixture);
  let failure = Reader::new(&client).head("o/r", 1).err().unwrap();
  assert_eq!(failure.code, "invalid_json");
  assert!(failure.message.contains("no head.sha"));
}

#[test]
fn github_failures_map_to_codes_classes_and_exits() {
  let status = |status| Error::Status {
    status,
    message: "boom".into(),
  };
  let class = |err: &Error| failure("pr", err).extra["class"].clone();
  assert_eq!(class(&status(404)), "permanent");
  assert_eq!(class(&status(502)), "transient");
  assert_eq!(class(&Error::Unauthorized), "unauthorized");
  assert_eq!(class(&Error::GraphQl(vec!["nope".into()])), "graphql");
  let decode = failure("pr", &Error::Decode("bad".into()));
  assert_eq!(decode.code, "invalid_json");
  let config = failure(
    "pr",
    &Error::Config("PB_GH_ATTEMPTS must be a number".into()),
  );
  assert_eq!(config.code, "usage");
  let token = toolu_github::TokenError::Unavailable {
    gh: "gh: not found".into(),
  };
  let unavailable = failure("head", &Error::Token(token));
  assert_eq!(
    (unavailable.code, unavailable.exit()),
    ("gh_unavailable", toolu_protocol::exit::Exit::Unavailable)
  );
  let limited = failure(
    "threads",
    &Error::RateLimited {
      status: 403,
      retry_after: Some(Duration::from_secs(60)),
      rate: RateLimit {
        reset: Some(1_760_000_000),
        remaining: Some(0),
        ..RateLimit::default()
      },
      secondary: false,
    },
  );
  assert_eq!(limited.extra["class"], "rate_limited");
  assert_eq!(limited.extra["resetAt"], 1_760_000_000);
  assert_eq!(limited.extra["retryAfter"], 60);
  assert_eq!(limited.extra["remaining"], 0);
  assert_eq!(limited.exit(), toolu_protocol::exit::Exit::TempFail);
}
