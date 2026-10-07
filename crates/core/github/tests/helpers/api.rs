//! The loopback GitHub API: a real TLS origin behind a CONNECT proxy, and
//! clients aimed at it.

use toolu_github::{Client, Config, Error};
use toolu_http_test_support::Fixture;
use toolu_runtime::env::Env;

/// A fixture origin standing in for `api.github.com`.
pub(crate) struct Api {
  /// The origin, its routes and what it observed.
  pub(crate) fixture: Fixture,
}

impl Api {
  /// Start the origin and its proxy.
  pub(crate) fn start() -> Result<Api, toolu_http_test_support::Error> {
    Ok(Api {
      fixture: Fixture::start()?,
    })
  }

  /// A client with `config`'s policy, aimed at this origin through its proxy.
  pub(crate) fn client(&self, mut config: Config, env: &Env) -> Result<Client, Error> {
    config.api_url = self.fixture.url("");
    config.http.test_root_ca_der = Some(self.fixture.root_ca_der().to_vec());
    let env = env.clone().with("HTTPS_PROXY", &self.fixture.proxy_url());
    Client::new(config, &env)
  }
}
