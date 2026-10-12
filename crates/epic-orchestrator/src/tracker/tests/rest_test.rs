use serde_json::json;
use toolu_http::{Auth, Method};
use toolu_http_test_support::{Fixture, Reply};

use super::Http;
use crate::support::{FAST, http_client};

#[test]
fn json_sends_the_body_with_its_content_type_and_decodes_the_reply() {
  let fixture = Fixture::start().expect("fixture");
  fixture
    .route("/api", Reply::new(200, r#"{"ok":1}"#))
    .expect("route");
  let http =
    Http::with_client(http_client(&fixture), FAST, "svc").authorized(Auth::Raw("key".into()));
  let reply = http
    .json(
      Method::Post,
      &fixture.url("/api"),
      Some(&json!({"a": 1})),
      true,
    )
    .expect("reply");
  assert_eq!(reply, json!({"ok": 1}));
  let requests = fixture.requests().expect("requests");
  assert_eq!(
    requests[0].headers.get("content-type").map(String::as_str),
    Some("application/json")
  );
  assert_eq!(
    requests[0].headers.get("authorization").map(String::as_str),
    Some("key")
  );
  assert_eq!(requests[0].body, br#"{"a":1}"#);
}

#[test]
fn an_empty_reply_is_null_and_bad_json_is_an_error() {
  let fixture = Fixture::start().expect("fixture");
  fixture
    .sequence(
      "/api",
      vec![Reply::new(204, ""), Reply::new(200, "not json")],
    )
    .expect("route");
  let http = Http::with_client(http_client(&fixture), FAST, "svc");
  let url = fixture.url("/api");
  assert_eq!(
    http.json(Method::Get, &url, None, false),
    Ok(serde_json::Value::Null)
  );
  let err = http.json(Method::Get, &url, None, false).expect_err("json");
  assert!(err.starts_with("svc: invalid JSON reply"), "{err}");
}

#[test]
fn an_error_body_that_echoes_a_secret_is_redacted() {
  let fixture = Fixture::start().expect("fixture");
  fixture
    .route("/api", Reply::new(401, "bad key sentinel-linear-key-123"))
    .expect("route");
  let http =
    Http::with_client(http_client(&fixture), FAST, "Linear").redacting("sentinel-linear-key-123");
  // A body, as every tracker POST has: the fixture's TLS reply to a body-less
  // POST can arrive as an EOF on Linux before the status is read.
  let query = serde_json::json!({"query": "{ viewer { id } }"});
  let err = http
    .json(Method::Post, &fixture.url("/api"), Some(&query), true)
    .expect_err("401");
  assert!(!err.contains("sentinel-linear-key-123"), "{err}");
  assert!(err.contains("[redacted]"), "{err}");
}
