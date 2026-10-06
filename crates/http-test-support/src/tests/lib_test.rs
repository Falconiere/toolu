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
