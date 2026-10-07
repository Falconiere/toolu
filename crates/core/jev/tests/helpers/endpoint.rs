//! The loopback Jev endpoint: a real TLS origin behind a CONNECT proxy, and
//! clients aimed at it with a 10 ms pause unit.

use std::time::Duration;

use serde_json::Value;
use toolu_http_test_support::Fixture;
use toolu_jev_client::{Config, Error, Jev};
use toolu_runtime::env::Env;

/// The path `jev.ts` posts to.
pub(crate) const PATH: &str = "/v1/systemone";

/// A fixture origin standing in for `api.typesafe.ai`.
pub(crate) struct Endpoint {
  /// The origin, its routes and what it observed.
  pub(crate) fixture: Fixture,
}

impl Endpoint {
  /// Start the origin and its proxy.
  pub(crate) fn start() -> Result<Endpoint, toolu_http_test_support::Error> {
    Ok(Endpoint {
      fixture: Fixture::start()?,
    })
  }

  /// A client with `TYPESAFE_API_KEY=fixture-key` and `extra` on top.
  pub(crate) fn jev(&self, extra: &[(&str, &str)]) -> Result<Jev, Error> {
    let mut env = Env::from_pairs([
      ("TYPESAFE_API_KEY", "fixture-key".to_owned()),
      ("HTTPS_PROXY", self.fixture.proxy_url()),
    ]);
    for (name, value) in extra {
      env = env.with(name, value);
    }
    let config = Config {
      endpoint: self.fixture.url(PATH),
      pause: Duration::from_millis(10),
      test_root_ca_der: Some(self.fixture.root_ca_der().to_vec()),
    };
    Jev::from_env(&env, config)
  }

  /// The bodies the endpoint received, as text.
  pub(crate) fn bodies(&self) -> Vec<String> {
    self
      .fixture
      .requests()
      .unwrap_or_default()
      .into_iter()
      .map(|request| String::from_utf8_lossy(&request.body).into_owned())
      .collect()
  }

  /// The last body received, parsed.
  pub(crate) fn last(&self) -> Value {
    self
      .bodies()
      .last()
      .and_then(|body| serde_json::from_str(body).ok())
      .unwrap_or(Value::Null)
  }
}
