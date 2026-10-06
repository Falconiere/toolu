use toolu_http::{Auth, Client, Config, Error};
use toolu_runtime::env::Env;

use crate::Fixture;

#[test]
fn missing_route_returns_real_http_404() {
  let fixture = Fixture::start().expect("fixture");
  let config = Config {
    test_root_ca_der: Some(fixture.root_ca_der().to_vec()),
    ..Config::default()
  };
  let env = Env::from_pairs([("HTTPS_PROXY", fixture.proxy_url())]);
  let client = Client::new(config, &env).expect("client");
  assert!(matches!(
    client.get_bytes(&fixture.url("/missing"), &Auth::None),
    Err(Error::HttpStatus(404))
  ));
}
