//! `Client::send` against the real loopback TLS origin and CONNECT proxy:
//! every status comes back as a `Response`, with its headers and body.

use toolu_http::{Auth, Client, Config, Error, Method, Request};
use toolu_http_test_support::{Fixture, Reply};
use toolu_runtime::env::Env;

fn client(fixture: &Fixture) -> Result<Client, Error> {
  let config = Config {
    test_root_ca_der: Some(fixture.root_ca_der().to_vec()),
    ..Config::default()
  };
  let env = Env::from_pairs([("HTTPS_PROXY", fixture.proxy_url())]);
  Client::new(config, &env)
}

fn get<'a>(url: &'a str, headers: &'a [(&'a str, &'a str)]) -> Request<'a> {
  Request {
    method: Method::Get,
    url,
    auth: &Auth::None,
    headers,
    body: None,
  }
}

#[test]
fn an_error_status_is_a_response_with_headers_and_body() {
  let fixture = Fixture::start().expect("fixture");
  fixture
    .route(
      "/missing",
      Reply::new(404, br#"{"message":"Not Found"}"#.to_vec()).header("X-GitHub-Request-Id", "r-1"),
    )
    .expect("route");
  let response = client(&fixture)
    .expect("client")
    .send(&get(&fixture.url("/missing"), &[("x-trace", "460")]))
    .expect("send");
  assert_eq!(response.status, 404);
  assert_eq!(response.header("x-github-request-id"), Some("r-1"));
  assert_eq!(response.body, br#"{"message":"Not Found"}"#);
  let requests = fixture.requests().expect("requests");
  assert_eq!(
    requests[0].headers.get("x-trace").map(String::as_str),
    Some("460")
  );
}

#[test]
fn not_modified_has_an_empty_body_and_the_request_carries_the_condition() {
  let fixture = Fixture::start().expect("fixture");
  fixture
    .route(
      "/etag",
      Reply::new(304, Vec::new()).header("ETag", "\"v1\""),
    )
    .expect("route");
  let response = client(&fixture)
    .expect("client")
    .send(&get(&fixture.url("/etag"), &[("If-None-Match", "\"v1\"")]))
    .expect("send");
  assert_eq!((response.status, response.body.len()), (304, 0));
  assert_eq!(response.header("ETag"), Some("\"v1\""));
  let requests = fixture.requests().expect("requests");
  assert_eq!(
    requests[0].headers.get("if-none-match").map(String::as_str),
    Some("\"v1\"")
  );
}

#[test]
fn a_body_goes_out_with_the_callers_content_type_and_a_patch_method() {
  let fixture = Fixture::start().expect("fixture");
  fixture
    .route("/patch", Reply::new(200, b"{}".to_vec()))
    .expect("route");
  let response = client(&fixture)
    .expect("client")
    .send(&Request {
      method: Method::Patch,
      url: &fixture.url("/patch"),
      auth: &Auth::Bearer("bearer-460".into()),
      headers: &[("Content-Type", "application/merge-patch+json")],
      body: Some(br#"{"a":1}"#),
    })
    .expect("send");
  assert_eq!(response.status, 200);
  let requests = fixture.requests().expect("requests");
  assert_eq!(requests[0].method, "PATCH");
  assert_eq!(requests[0].body, br#"{"a":1}"#);
  assert_eq!(
    requests[0].headers.get("content-type").map(String::as_str),
    Some("application/merge-patch+json")
  );
  assert_eq!(
    requests[0].headers.get("authorization").map(String::as_str),
    Some("Bearer bearer-460")
  );
}

#[test]
fn zero_redirects_return_the_redirect_itself() {
  let fixture = Fixture::start().expect("fixture");
  fixture
    .route(
      "/moved",
      Reply::new(302, Vec::new()).header("Location", &fixture.url("/target")),
    )
    .expect("route");
  let config = Config {
    max_redirects: 0,
    test_root_ca_der: Some(fixture.root_ca_der().to_vec()),
    ..Config::default()
  };
  let env = Env::from_pairs([("HTTPS_PROXY", fixture.proxy_url())]);
  let response = Client::new(config, &env)
    .expect("client")
    .send(&get(&fixture.url("/moved"), &[]))
    .expect("send");
  assert_eq!(response.status, 302);
  assert_eq!(fixture.requests().expect("requests").len(), 1);
}
