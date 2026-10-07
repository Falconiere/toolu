//! REST calls against the loopback API: conditional requests and their cost,
//! the request headers, pagination links, foreign paths and redirects.

#[path = "helpers/api.rs"]
mod api;

use api::Api;
use serde_json::{Value, json};
use toolu_github::{Config, Error, Method, Rest, RestRequest};
use toolu_http_test_support::Reply;
use toolu_runtime::env::Env;

fn env() -> Env {
  Env::from_pairs([("GH_TOKEN", "rest-token")])
}

#[test]
fn an_etag_makes_the_next_get_conditional_and_free() {
  let api = Api::start().expect("api");
  let client = api.client(Config::scheduled(), &env()).expect("client");
  api
    .fixture
    .route(
      "/repos/o/r/pulls/1",
      Reply::new(200, r#"{"number":1}"#)
        .header("ETag", "\"v1\"")
        .header("X-RateLimit-Resource", "core")
        .header("X-RateLimit-Remaining", "4999"),
    )
    .expect("route");
  let first = client.get("/repos/o/r/pulls/1", None).expect("first");
  let Rest::Fresh(fresh) = &first.data else {
    panic!("expected a body: {:?}", first.data);
  };
  assert_eq!(fresh.etag.as_deref(), Some("\"v1\""));
  assert_eq!(fresh.json::<Value>().expect("json")["number"], 1);
  assert_eq!(first.cost.points, Some(1));
  assert_eq!(first.cost.rate.remaining, Some(4999));
  api
    .fixture
    .route("/repos/o/r/pulls/1", Reply::new(304, ""))
    .expect("route");
  let second = client
    .get("/repos/o/r/pulls/1", fresh.etag.as_deref())
    .expect("second");
  assert_eq!(
    (second.data, second.cost.points),
    (Rest::NotModified, Some(0))
  );
  let requests = api.fixture.requests().expect("requests");
  let header = |index: usize, name: &str| requests[index].headers.get(name).cloned();
  assert_eq!(header(0, "if-none-match"), None);
  assert_eq!(header(1, "if-none-match").as_deref(), Some("\"v1\""));
  assert_eq!(
    header(0, "accept").as_deref(),
    Some("application/vnd.github+json")
  );
  assert_eq!(
    header(0, "x-github-api-version").as_deref(),
    Some("2022-11-28")
  );
  assert!(header(0, "user-agent").is_some_and(|agent| agent.starts_with("toolu/")));
  assert_eq!(
    header(0, "authorization").as_deref(),
    Some("Bearer rest-token")
  );
}

#[test]
fn a_not_modified_without_an_etag_is_still_not_modified() {
  let api = Api::start().expect("api");
  let client = api.client(Config::scheduled(), &env()).expect("client");
  api.fixture.route("/x", Reply::new(304, "")).expect("route");
  assert_eq!(
    client.get("/x", None).expect("reply").data,
    Rest::NotModified
  );
}

#[test]
fn a_pagination_link_on_the_same_origin_is_followed() {
  let api = Api::start().expect("api");
  let client = api.client(Config::scheduled(), &env()).expect("client");
  api
    .fixture
    .route("/repos/o/r/pulls?page=2", Reply::new(200, "[]"))
    .expect("route");
  let link = api.fixture.url("/repos/o/r/pulls?page=2");
  assert!(matches!(
    client.get(&link, None).expect("page").data,
    Rest::Fresh(_)
  ));
}

#[test]
fn a_path_off_the_api_sends_nothing() {
  let api = Api::start().expect("api");
  let client = api.client(Config::scheduled(), &env()).expect("client");
  for path in ["https://api.github.com.evil.example/x", "//evil.example/x"] {
    assert!(
      matches!(client.get(path, None), Err(Error::Config(_))),
      "{path}"
    );
  }
  let http = Config {
    api_url: "http://api.example.test".into(),
    ..Config::scheduled()
  };
  assert!(matches!(
    toolu_github::Client::new(http, &env()),
    Err(Error::Config(_))
  ));
  assert_eq!(api.fixture.requests().expect("requests").len(), 0);
}

#[test]
fn a_body_is_sent_as_json() {
  let api = Api::start().expect("api");
  let client = api.client(Config::scheduled(), &env()).expect("client");
  api
    .fixture
    .route("/repos/o/r/issues/1/labels", Reply::new(200, "[]"))
    .expect("route");
  let body = json!({ "labels": ["merge-approved"] });
  client
    .rest(&RestRequest {
      method: Method::Post,
      path: "/repos/o/r/issues/1/labels",
      body: Some(&body),
      etag: None,
    })
    .expect("post");
  let requests = api.fixture.requests().expect("requests");
  assert_eq!(requests[0].method, "POST");
  assert_eq!(
    serde_json::from_slice::<Value>(&requests[0].body).expect("body"),
    body
  );
  assert_eq!(
    requests[0].headers.get("content-type").map(String::as_str),
    Some("application/json")
  );
}

#[test]
fn a_redirect_reaches_the_second_origin_without_the_token() {
  let api = Api::start().expect("api");
  let client = api.client(Config::scheduled(), &env()).expect("client");
  let target = api.fixture.second_url("/moved");
  api
    .fixture
    .route(
      "/repos/old/r",
      Reply::new(301, "").header("Location", &target),
    )
    .expect("route");
  api
    .fixture
    .route("/moved", Reply::new(200, "{}"))
    .expect("route");
  assert!(matches!(
    client.get("/repos/old/r", None).expect("moved").data,
    Rest::Fresh(_)
  ));
  let requests = api.fixture.requests().expect("requests");
  assert_eq!(requests.len(), 2);
  assert!(!requests[1].headers.contains_key("authorization"));
}

#[test]
fn a_body_over_the_cap_is_not_retried() {
  let api = Api::start().expect("api");
  let mut config =
    Config::one_shot(&Env::from_pairs([("PB_GH_BACKOFF", "0 0 0")])).expect("config");
  config.http.max_body_bytes = 16;
  let client = api.client(config, &env()).expect("client");
  api
    .fixture
    .route("/big", Reply::new(200, vec![b'a'; 64]))
    .expect("route");
  assert_eq!(
    client.get("/big", None),
    Err(Error::Transport(toolu_http::Error::BodyTooLarge))
  );
  assert_eq!(api.fixture.requests().expect("requests").len(), 1);
}
