use serde_json::Value;
use toolu_http::{Auth, Client, Config};
use toolu_runtime::env::Env;

use crate::{Fixture, Reply};

#[test]
fn serves_real_tls_through_connect_proxy() {
  let fixture = Fixture::start().expect("fixture starts");
  fixture
    .route("/one", Reply::new(200, br#"{"ok":true}"#.to_vec()))
    .expect("route");
  let config = Config {
    test_root_ca_der: Some(fixture.root_ca_der().to_vec()),
    ..Config::default()
  };
  let env = Env::from_pairs([("HTTPS_PROXY", fixture.proxy_url())]);
  let client = Client::new(config, &env).expect("client");
  let result: Value = client
    .get_json(&fixture.url("/one"), &Auth::None)
    .expect("response");
  assert_eq!(result["ok"], true);
  assert_eq!(fixture.requests().expect("requests")[0].path, "/one");
  assert!(fixture.connects().expect("connects")[0].starts_with("api.example.test:"));
}

fn client(fixture: &Fixture) -> Client {
  let config = Config {
    test_root_ca_der: Some(fixture.root_ca_der().to_vec()),
    ..Config::default()
  };
  let env = Env::from_pairs([("HTTPS_PROXY", fixture.proxy_url())]);
  Client::new(config, &env).expect("client")
}

#[test]
fn a_sequence_answers_in_order_then_repeats_its_last_reply() {
  let fixture = Fixture::start().expect("fixture");
  fixture
    .sequence(
      "/seq",
      vec![Reply::new(500, "first"), Reply::new(200, "second")],
    )
    .expect("sequence");
  let client = client(&fixture);
  let url = fixture.url("/seq");
  assert_eq!(
    client.get_bytes(&url, &Auth::None),
    Err(toolu_http::Error::HttpStatus(500))
  );
  assert_eq!(
    client.get_bytes(&url, &Auth::None).expect("second"),
    b"second"
  );
  assert_eq!(
    client.get_bytes(&url, &Auth::None).expect("repeat"),
    b"second"
  );
  assert_eq!(fixture.requests().expect("requests").len(), 3);
}

#[test]
fn a_dropped_reply_closes_the_connection_and_records_nothing() {
  let fixture = Fixture::start().expect("fixture");
  fixture
    .sequence("/drop", vec![Reply::dropped(), Reply::new(200, "ok")])
    .expect("sequence");
  let client = client(&fixture);
  let url = fixture.url("/drop");
  assert!(matches!(
    client.get_bytes(&url, &Auth::None),
    Err(toolu_http::Error::Transport(_))
  ));
  assert_eq!(
    client.get_bytes(&url, &Auth::None).expect("after drop"),
    b"ok"
  );
  assert_eq!(fixture.requests().expect("requests").len(), 1);
  assert_eq!(fixture.connects().expect("connects").len(), 2);
}

#[test]
fn an_empty_sequence_removes_the_route() {
  let fixture = Fixture::start().expect("fixture");
  fixture.route("/gone", Reply::new(200, "x")).expect("route");
  fixture.sequence("/gone", Vec::new()).expect("sequence");
  assert_eq!(
    client(&fixture).get_bytes(&fixture.url("/gone"), &Auth::None),
    Err(toolu_http::Error::HttpStatus(404))
  );
}
