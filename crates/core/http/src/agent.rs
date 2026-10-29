//! Lazily retained `ureq::Agent`s per URL scheme for one [`Client`](crate::Client).

use std::sync::{Arc, Mutex};

use ureq::config::RedirectAuthHeaders;
use ureq::tls::{Certificate, RootCerts, TlsConfig, TlsProvider};

use crate::error::Error;
use crate::{Client, proxy_for};

/// Shared scheme agents for one client (and its `Clone` / `with_timeout` views).
#[derive(Default)]
pub(crate) struct AgentPool {
  http: Mutex<Option<ureq::Agent>>,
  https: Mutex<Option<ureq::Agent>>,
}

impl Client {
  /// The retained Agent for `scheme`, built on first use.
  pub(crate) fn agent_for(&self, scheme: &str) -> Result<ureq::Agent, Error> {
    let slot = if scheme == "https" {
      &self.agents.https
    } else {
      &self.agents.http
    };
    let mut guard = slot
      .lock()
      .map_err(|_poison| Error::InvalidConfig("agent pool lock poisoned".into()))?;
    if let Some(agent) = guard.as_ref() {
      return Ok(agent.clone());
    }
    let agent = self.build_agent(scheme)?;
    *guard = Some(agent.clone());
    Ok(agent)
  }

  fn build_agent(&self, scheme: &str) -> Result<ureq::Agent, Error> {
    let certs = match &self.config.test_root_ca_der {
      Some(der) => RootCerts::new_with_certs(&[Certificate::from_der(der).to_owned()]),
      None => RootCerts::WebPki,
    };
    let tls = TlsConfig::builder()
      .provider(TlsProvider::Rustls)
      .root_certs(certs)
      .unversioned_rustls_crypto_provider(Arc::new(rustls::crypto::ring::default_provider()))
      .build();
    let proxy = proxy_for(&self.env, scheme)
      .map(ureq::Proxy::new)
      .transpose()
      .map_err(|err| Error::InvalidConfig(err.to_string()))?;
    let config = ureq::Agent::config_builder()
      .timeout_global(Some(self.config.timeout))
      .max_redirects(self.config.max_redirects)
      .redirect_auth_headers(RedirectAuthHeaders::Never)
      .http_status_as_error(false)
      .proxy(proxy)
      .tls_config(tls)
      .build();
    Ok(ureq::Agent::new_with_config(config))
  }
}

impl AgentPool {
  /// Empty shared pool for a newly constructed client.
  pub(crate) fn new() -> Arc<Self> {
    Arc::new(Self::default())
  }
}

#[cfg(test)]
#[path = "tests/agent_test.rs"]
mod tests;
