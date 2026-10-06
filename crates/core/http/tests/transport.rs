//! Real TLS and CONNECT integration coverage for the public transport.

use std::time::Duration;

use serde_json::{Value, json};
use toolu_http::{Auth, Client, Config, Error};
use toolu_http_test_support::{Fixture, Reply};
use toolu_runtime::env::Env;

fn client(fixture: &Fixture, config: &Config) -> Result<Client, Error> {
  let config = Config {
    timeout: config.timeout,
    max_body_bytes: config.max_body_bytes,
    max_redirects: config.max_redirects,
    test_root_ca_der: Some(fixture.root_ca_der().to_vec()),
  };
  let env = Env::from_pairs([("HTTPS_PROXY", fixture.proxy_url())]);
  Client::new(config, &env)
}

#[test]
fn get_json_sends_basic_auth_through_connect() {
  let fixture = Fixture::start().expect("fixture");
  fixture
    .route("/get", Reply::new(200, br#"{"kind":"get"}"#.to_vec()))
    .expect("route");
  let client = client(&fixture, &Config::default()).expect("client");
  let auth = Auth::Basic {
    username: "alice".into(),
    password: "secret".into(),
  };
  let result: Value = client
    .get_json(&fixture.url("/get"), &auth)
    .unwrap_or_else(|err| {
      panic!(
        "GET: {err:?}; CONNECTs: {:?}; HTTPS requests: {:?}",
        fixture.connects(),
        fixture.requests()
      )
    });
  assert_eq!(result["kind"], "get");
  let requests: Vec<toolu_http_test_support::ObservedRequest> =
    fixture.requests().expect("requests");
  assert_eq!(requests[0].method, "GET");
  assert_eq!(
    requests[0].headers.get("authorization").map(String::as_str),
    Some("Basic YWxpY2U6c2VjcmV0")
  );
  assert!(fixture.connects().expect("connects")[0].starts_with("api.example.test:"));
}

#[test]
fn post_and_put_send_json_and_bearer_auth() {
  let fixture = Fixture::start().expect("fixture");
  fixture
    .route("/post", Reply::new(200, br#"{"kind":"post"}"#.to_vec()))
    .expect("route");
  fixture
    .route("/put", Reply::new(200, br#"{"kind":"put"}"#.to_vec()))
    .expect("route");
  let client = client(&fixture, &Config::default()).expect("client");
  let post: Value = client
    .post_json(
      &fixture.url("/post"),
      &Auth::Bearer("token-417".into()),
      &json!({"x":1}),
    )
    .expect("POST");
  let put: Value = client
    .put_json(&fixture.url("/put"), &Auth::None, &json!({"y":2}))
    .expect("PUT");
  assert_eq!(
    (post["kind"].as_str(), put["kind"].as_str()),
    (Some("post"), Some("put"))
  );
  let requests = fixture.requests().expect("requests");
  assert_eq!(
    (requests[0].method.as_str(), requests[1].method.as_str()),
    ("POST", "PUT")
  );
  assert_eq!(
    requests[0].headers.get("authorization").map(String::as_str),
    Some("Bearer token-417")
  );
  assert_eq!(
    requests[0].headers.get("content-type").map(String::as_str),
    Some("application/json")
  );
  assert_eq!(
    serde_json::from_slice::<Value>(&requests[0].body).expect("POST body"),
    json!({"x":1})
  );
  assert_eq!(
    serde_json::from_slice::<Value>(&requests[1].body).expect("PUT body"),
    json!({"y":2})
  );
}

#[test]
fn test_ca_is_required_and_proxy_records_the_connect_target() {
  let fixture = Fixture::start().expect("fixture");
  fixture
    .route("/ok", Reply::new(200, b"ok".to_vec()))
    .expect("route");
  let client = client(&fixture, &Config::default()).expect("client");
  assert_eq!(
    client
      .get_bytes(&fixture.url("/ok"), &Auth::None)
      .expect("trusted TLS"),
    b"ok"
  );
  assert!(
    fixture
      .connects()
      .expect("connects")
      .iter()
      .any(|value| value.starts_with("api.example.test:"))
  );
  let env = Env::from_pairs([("HTTPS_PROXY", fixture.proxy_url())]);
  let without_ca = Client::new(Config::default(), &env).expect("client");
  assert!(matches!(
    without_ca.get_bytes(&fixture.url("/ok"), &Auth::None),
    Err(Error::Transport(_))
  ));
}

#[test]
fn timeout_and_oversize_bodies_have_typed_errors() {
  let fixture = Fixture::start().expect("fixture");
  fixture
    .route(
      "/slow",
      Reply::new(200, b"late".to_vec()).delayed(Duration::from_millis(200)),
    )
    .expect("route");
  fixture
    .route("/large", Reply::new(200, vec![b'a'; 16]))
    .expect("route");
  fixture
    .route("/error", Reply::new(404, b"missing".to_vec()))
    .expect("route");
  let short = client(
    &fixture,
    &Config {
      timeout: Duration::from_millis(50),
      ..Config::default()
    },
  )
  .expect("client");
  assert!(matches!(
    short.get_bytes(&fixture.url("/slow"), &Auth::None),
    Err(Error::Timeout)
  ));
  let small = client(
    &fixture,
    &Config {
      max_body_bytes: 4,
      ..Config::default()
    },
  )
  .expect("client");
  assert!(matches!(
    small.get_bytes(&fixture.url("/large"), &Auth::None),
    Err(Error::BodyTooLarge)
  ));
  assert!(matches!(
    small.get_bytes(&fixture.url("/error"), &Auth::None),
    Err(Error::BodyTooLarge)
  ));
}

#[test]
fn status_decode_and_connection_failures_have_typed_errors() {
  let fixture = Fixture::start().expect("fixture");
  fixture
    .route("/error", Reply::new(404, b"missing".to_vec()))
    .expect("route");
  fixture
    .route("/bad-json", Reply::new(200, b"not json".to_vec()))
    .expect("route");
  let client = client(&fixture, &Config::default()).expect("client");
  assert!(matches!(
    client.get_bytes(&fixture.url("/error"), &Auth::None),
    Err(Error::HttpStatus(404))
  ));
  assert!(matches!(
    client.get_json::<Value>(&fixture.url("/bad-json"), &Auth::None),
    Err(Error::Decode(_))
  ));
  assert!(matches!(
    client.get_bytes("http://127.0.0.1:1/", &Auth::None),
    Err(Error::Transport(_))
  ));
}

#[test]
fn redirect_to_second_origin_drops_authorization() {
  let fixture = Fixture::start().expect("fixture");
  fixture
    .route(
      "/redirect",
      Reply::new(302, Vec::new()).header("Location", &fixture.second_url("/download")),
    )
    .expect("route");
  fixture
    .route("/download", Reply::new(200, b"downloaded".to_vec()))
    .expect("route");
  let client = client(&fixture, &Config::default()).expect("client");
  let bytes = client
    .get_bytes(&fixture.url("/redirect"), &Auth::Bearer("secret".into()))
    .expect("redirect");
  assert_eq!(bytes, b"downloaded");
  let requests = fixture.requests().expect("requests");
  assert_eq!(requests.len(), 2);
  assert_eq!(
    requests[0].headers.get("authorization").map(String::as_str),
    Some("Bearer secret")
  );
  assert!(!requests[1].headers.contains_key("authorization"));
  assert_eq!(fixture.connects().expect("connects").len(), 2);
}
