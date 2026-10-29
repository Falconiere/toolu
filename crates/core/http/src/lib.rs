//! Small synchronous HTTP transport for the Rust Jev and GitHub clients.
//! Every request has a deadline and a bounded response body. TLS uses rustls
//! with the `ring` crypto provider. One client retains a ureq Agent per scheme
//! so eligible requests can reuse connections.

use std::fmt;
use std::io::Read;
use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use serde::de::DeserializeOwned;
use toolu_runtime::env::Env;

mod agent;
mod auth;
mod error;
mod pem;
mod send;

pub use auth::Auth;
pub use error::Error;
use error::map_io;
pub use pem::{pem_file_to_der, pem_to_der};
pub use send::{Method, Request, Response, check_url};

use agent::AgentPool;

/// This crate's layer in `tooling/conventions/guardrails/rust/layers.json`.
pub const LAYER: &str = "http";

/// Settings applied to every request made by a client.
#[derive(Debug, Clone)]
pub struct Config {
  /// Whole-request deadline, including redirects and body reading.
  pub timeout: Duration,
  /// Maximum response bytes, including an HTTP error body.
  pub max_body_bytes: usize,
  /// Maximum redirect hops; 0 returns a 3xx response as it is.
  pub max_redirects: u32,
  /// DER-encoded root certificate used only by loopback tests.
  pub test_root_ca_der: Option<Vec<u8>>,
}

impl Default for Config {
  fn default() -> Self {
    Self {
      timeout: Duration::from_secs(30),
      max_body_bytes: 1024 * 1024,
      max_redirects: 5,
      test_root_ca_der: None,
    }
  }
}

/// A blocking client with explicit environment and a per-request deadline.
///
/// `Clone` and [`Self::with_timeout`] share the retained scheme Agents. A new
/// [`Self::new`] gets its own Agents so proxy and TLS settings stay isolated.
pub struct Client {
  pub(crate) config: Config,
  pub(crate) env: Env,
  pub(crate) agents: Arc<AgentPool>,
}

impl Clone for Client {
  fn clone(&self) -> Self {
    Self {
      config: self.config.clone(),
      env: self.env.clone(),
      agents: Arc::clone(&self.agents),
    }
  }
}

impl fmt::Debug for Client {
  /// The configuration only: the environment may hold credentials.
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("Client")
      .field("config", &self.config)
      .finish_non_exhaustive()
  }
}

impl Client {
  /// Build a client. Proxy values are read from `env`, never from the process.
  ///
  /// # Errors
  /// Returns `InvalidConfig` for zero limits, a malformed selected proxy, or
  /// an invalid test root certificate.
  pub fn new(config: Config, env: &Env) -> Result<Self, Error> {
    if config.timeout.is_zero() {
      return Err(Error::InvalidConfig("timeout must be nonzero".into()));
    }
    if config.max_body_bytes == 0 {
      return Err(Error::InvalidConfig("body limit must be nonzero".into()));
    }
    for scheme in ["http", "https"] {
      if let Some(proxy) = proxy_for(env, scheme) {
        ureq::Proxy::new(proxy).map_err(|err| Error::InvalidConfig(err.to_string()))?;
      }
    }
    if let Some(der) = &config.test_root_ca_der {
      rustls::RootCertStore::empty()
        .add(rustls::pki_types::CertificateDer::from(der.clone()))
        .map_err(|err| Error::InvalidConfig(format!("invalid test root CA: {err}")))?;
    }
    Ok(Self {
      config,
      env: env.clone(),
      agents: AgentPool::new(),
    })
  }

  /// Same Agent pools with a different whole-request timeout.
  ///
  /// # Errors
  /// Returns `InvalidConfig` when `timeout` is zero.
  pub fn with_timeout(&self, timeout: Duration) -> Result<Self, Error> {
    if timeout.is_zero() {
      return Err(Error::InvalidConfig("timeout must be nonzero".into()));
    }
    Ok(Self {
      config: Config {
        timeout,
        max_body_bytes: self.config.max_body_bytes,
        max_redirects: self.config.max_redirects,
        test_root_ca_der: self.config.test_root_ca_der.clone(),
      },
      env: self.env.clone(),
      agents: Arc::clone(&self.agents),
    })
  }

  /// Download a bounded response, following redirects without forwarding auth.
  ///
  /// # Errors
  /// Returns a typed status, timeout, size, or transport error.
  pub fn get_bytes(&self, url: &str, auth: &Auth) -> Result<Vec<u8>, Error> {
    self.checked(Method::Get, url, auth, None)
  }

  /// Decode a bounded JSON GET response.
  ///
  /// # Errors
  /// Returns a typed request or JSON decode error.
  pub fn get_json<T: DeserializeOwned>(&self, url: &str, auth: &Auth) -> Result<T, Error> {
    let bytes = self.get_bytes(url, auth)?;
    decode(&bytes)
  }

  /// Send JSON in a POST and decode the bounded JSON response.
  ///
  /// # Errors
  /// Returns a typed request, JSON encode, or JSON decode error.
  pub fn post_json<B: Serialize, T: DeserializeOwned>(
    &self,
    url: &str,
    auth: &Auth,
    body: &B,
  ) -> Result<T, Error> {
    self.send_json(Method::Post, url, auth, body)
  }

  /// Send JSON in a PUT and decode the bounded JSON response.
  ///
  /// # Errors
  /// Returns a typed request, JSON encode, or JSON decode error.
  pub fn put_json<B: Serialize, T: DeserializeOwned>(
    &self,
    url: &str,
    auth: &Auth,
    body: &B,
  ) -> Result<T, Error> {
    self.send_json(Method::Put, url, auth, body)
  }

  fn send_json<B: Serialize, T: DeserializeOwned>(
    &self,
    method: Method,
    url: &str,
    auth: &Auth,
    body: &B,
  ) -> Result<T, Error> {
    let body = serde_json::to_vec(body).map_err(|err| Error::Encode(err.to_string()))?;
    let response = self.checked(method, url, auth, Some(&body))?;
    decode(&response)
  }

  /// `send`, then a status of 400 or more as `HttpStatus`.
  fn checked(
    &self,
    method: Method,
    url: &str,
    auth: &Auth,
    body: Option<&[u8]>,
  ) -> Result<Vec<u8>, Error> {
    let headers: &[(&str, &str)] = match body {
      Some(_) => &[("content-type", "application/json")],
      None => &[],
    };
    let response = self.send(&Request {
      method,
      url,
      auth,
      headers,
      body,
    })?;
    if response.status >= 400 {
      return Err(Error::HttpStatus(response.status));
    }
    Ok(response.body)
  }

  /// Bound the response body and collect status plus headers.
  pub(crate) fn read_response(
    &self,
    mut response: ureq::http::Response<ureq::Body>,
  ) -> Result<Response, Error> {
    let status = response.status().as_u16();
    let headers = response
      .headers()
      .iter()
      .map(|(name, value)| {
        let value = String::from_utf8_lossy(value.as_bytes()).into_owned();
        (name.as_str().to_owned(), value)
      })
      .collect();
    let max = u64::try_from(self.config.max_body_bytes)
      .map_err(|err| Error::InvalidConfig(err.to_string()))?;
    let mut reader = response
      .body_mut()
      .with_config()
      .reader()
      .take(max.saturating_add(1));
    let mut body = Vec::new();
    reader.read_to_end(&mut body).map_err(|err| map_io(&err))?;
    if body.len() > self.config.max_body_bytes {
      return Err(Error::BodyTooLarge);
    }
    Ok(Response {
      status,
      headers,
      body,
    })
  }
}

fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, Error> {
  serde_json::from_slice(bytes).map_err(|err| Error::Decode(err.to_string()))
}

/// Proxy URL for `scheme` from the explicit environment snapshot.
pub(crate) fn proxy_for<'a>(env: &'a Env, scheme: &str) -> Option<&'a str> {
  let names = if scheme == "https" {
    ["HTTPS_PROXY", "https_proxy"]
  } else {
    ["HTTP_PROXY", "http_proxy"]
  };
  names
    .into_iter()
    .chain(["ALL_PROXY", "all_proxy"])
    .find_map(|name| env.get(name))
}

/// Test-only rustls types for the reusable in-process fixture crate.
#[cfg(feature = "test-support")]
pub use rustls as fixture_rustls;

#[cfg(test)]
#[path = "tests/lib_test.rs"]
mod tests;
