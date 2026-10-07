use toolu_runtime::env::Env;

use crate::{Auth, Client, Config, Error, Method, Request, Response};

fn response() -> Response {
  Response {
    status: 404,
    headers: vec![
      ("x-test".into(), "first".into()),
      ("x-test".into(), "second".into()),
      ("etag".into(), "\"abc\"".into()),
    ],
    body: b"missing".to_vec(),
  }
}

#[test]
fn header_lookup_ignores_case_and_returns_the_first_value() {
  let response = response();
  assert_eq!(response.header("ETag"), Some("\"abc\""));
  assert_eq!(response.header("X-TEST"), Some("first"));
  assert_eq!(response.header("retry-after"), None);
}

#[test]
fn methods_map_to_their_http_names() {
  let names: Vec<String> = [
    Method::Get,
    Method::Post,
    Method::Put,
    Method::Patch,
    Method::Delete,
  ]
  .into_iter()
  .map(|method| method.http().to_string())
  .collect();
  assert_eq!(names, ["GET", "POST", "PUT", "PATCH", "DELETE"]);
}

#[test]
fn credential_headers_are_refused_before_any_connection() {
  let client = Client::new(Config::default(), &Env::default()).expect("client");
  for name in ["Authorization", "COOKIE", "proxy-authorization"] {
    let headers = [(name, "secret")];
    let result = client.send(&Request {
      method: Method::Get,
      url: "http://127.0.0.1:1/",
      auth: &Auth::None,
      headers: &headers,
      body: None,
    });
    assert_eq!(
      result,
      Err(Error::InvalidConfig(format!(
        "header {name} is refused; credentials go through Auth"
      )))
    );
  }
}

#[test]
fn a_url_without_an_http_scheme_and_host_is_a_transport_error() {
  let client = Client::new(Config::default(), &Env::default()).expect("client");
  for url in ["ftp://example.test/x", "/relative", "https://[broken"] {
    let result = client.send(&Request {
      method: Method::Get,
      url,
      auth: &Auth::None,
      headers: &[],
      body: None,
    });
    assert!(
      matches!(result, Err(Error::Transport(_))),
      "{url}: {result:?}"
    );
  }
}
