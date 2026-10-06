//! Small synchronous HTTP transport for the Rust Jev and GitHub clients.
//! Every request has a deadline and a bounded response body. TLS uses rustls
//! with the `ring` crypto provider.

use std::fmt;
use std::io::Read;
use std::sync::Arc;
use std::time::Duration;

use base64::Engine;
use serde::Serialize;
use serde::de::DeserializeOwned;
use toolu_runtime::env::Env;
use ureq::config::RedirectAuthHeaders;
use ureq::tls::{Certificate, RootCerts, TlsConfig, TlsProvider};

/// This crate's layer in `tooling/conventions/guardrails/rust/layers.json`.
pub const LAYER: &str = "http";

/// Settings applied to every request made by a client.
#[derive(Debug, Clone)]
pub struct Config {
  /// Whole-request deadline, including redirects and body reading.
  pub timeout: Duration,
  /// Maximum response bytes, including an HTTP error body.
  pub max_body_bytes: usize,
  /// Maximum redirect hops.
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

/// Authentication attached to the initial request only.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Auth {
  /// No Authorization header.
  #[default]
  None,
  /// HTTP basic authentication.
  Basic {
    /// Username, possibly empty.
    username: String,
    /// Password, possibly empty.
    password: String,
  },
  /// HTTP bearer authentication.
  Bearer(String),
}

impl Auth {
  fn header_value(&self) -> Option<String> {
    match self {
      Self::None => None,
      Self::Basic { username, password } => {
        let encoded =
          base64::engine::general_purpose::STANDARD.encode(format!("{username}:{password}"));
        Some(format!("Basic {encoded}"))
      }
      Self::Bearer(token) => Some(format!("Bearer {token}")),
    }
  }
}

/// A caller, server, timeout, size, JSON, or transport error.
#[derive(Debug, PartialEq, Eq)]
pub enum Error {
  /// An invalid client setting, such as a zero timeout or malformed proxy.
  InvalidConfig(String),
  /// An HTTP 4xx or 5xx status.
  HttpStatus(u16),
  /// The whole-request deadline expired.
  Timeout,
  /// A response exceeded `Config::max_body_bytes`.
  BodyTooLarge,
  /// JSON request serialization failed.
  Encode(String),
  /// JSON response deserialization failed.
  Decode(String),
  /// A URL, TLS, proxy, redirect, or network error.
  Transport(String),
}

impl fmt::Display for Error {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::InvalidConfig(reason) => write!(f, "invalid HTTP configuration: {reason}"),
      Self::HttpStatus(status) => write!(f, "HTTP status {status}"),
      Self::Timeout => f.write_str("HTTP request timed out"),
      Self::BodyTooLarge => f.write_str("HTTP response body exceeds limit"),
      Self::Encode(reason) => write!(f, "cannot encode JSON request: {reason}"),
      Self::Decode(reason) => write!(f, "cannot decode JSON response: {reason}"),
      Self::Transport(reason) => write!(f, "HTTP transport error: {reason}"),
    }
  }
}

impl std::error::Error for Error {}

/// A blocking client with explicit environment and a per-request deadline.
#[derive(Debug, Clone)]
pub struct Client {
  config: Config,
  env: Env,
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
    })
  }

  /// Download a bounded response, following redirects without forwarding auth.
  ///
  /// # Errors
  /// Returns a typed status, timeout, size, or transport error.
  pub fn get_bytes(&self, url: &str, auth: &Auth) -> Result<Vec<u8>, Error> {
    self.request(ureq::http::Method::GET, url, auth, None)
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
    self.send_json(ureq::http::Method::POST, url, auth, body)
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
    self.send_json(ureq::http::Method::PUT, url, auth, body)
  }

  fn send_json<B: Serialize, T: DeserializeOwned>(
    &self,
    method: ureq::http::Method,
    url: &str,
    auth: &Auth,
    body: &B,
  ) -> Result<T, Error> {
    let body = serde_json::to_vec(body).map_err(|err| Error::Encode(err.to_string()))?;
    let response = self.request(method, url, auth, Some(body))?;
    decode(&response)
  }

  fn request(
    &self,
    method: ureq::http::Method,
    url: &str,
    auth: &Auth,
    body: Option<Vec<u8>>,
  ) -> Result<Vec<u8>, Error> {
    let uri: ureq::http::Uri = url.parse().map_err(|err: ureq::http::uri::InvalidUri| {
      Error::Transport(format!("invalid URL: {err}"))
    })?;
    let scheme = uri
      .scheme_str()
      .ok_or_else(|| Error::Transport("URL has no scheme".into()))?;
    if !matches!(scheme, "http" | "https") || uri.host().is_none() {
      return Err(Error::Transport(
        "URL must have an HTTP(S) scheme and host".into(),
      ));
    }
    let agent = self.agent(scheme)?;
    let mut builder = ureq::http::Request::builder().method(method).uri(uri);
    if let Some(value) = auth.header_value() {
      builder = builder.header(ureq::http::header::AUTHORIZATION, value);
    }
    let response = match body {
      Some(bytes) => agent.run(
        builder
          .header(ureq::http::header::CONTENT_TYPE, "application/json")
          .body(bytes)
          .map_err(|err| map_http(&err))?,
      ),
      None => agent.run(builder.body(()).map_err(|err| map_http(&err))?),
    }
    .map_err(|err| map_ureq(&err))?;
    self.read_response(response)
  }

  fn agent(&self, scheme: &str) -> Result<ureq::Agent, Error> {
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

  fn read_response(
    &self,
    mut response: ureq::http::Response<ureq::Body>,
  ) -> Result<Vec<u8>, Error> {
    let status = response.status().as_u16();
    let max = u64::try_from(self.config.max_body_bytes)
      .map_err(|err| Error::InvalidConfig(err.to_string()))?;
    let mut reader = response
      .body_mut()
      .with_config()
      .reader()
      .take(max.saturating_add(1));
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes).map_err(|err| map_io(&err))?;
    if bytes.len() > self.config.max_body_bytes {
      return Err(Error::BodyTooLarge);
    }
    if status >= 400 {
      return Err(Error::HttpStatus(status));
    }
    Ok(bytes)
  }
}

fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, Error> {
  serde_json::from_slice(bytes).map_err(|err| Error::Decode(err.to_string()))
}

fn proxy_for<'a>(env: &'a Env, scheme: &str) -> Option<&'a str> {
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

fn map_http(err: &ureq::http::Error) -> Error {
  Error::Transport(err.to_string())
}

fn map_ureq(err: &ureq::Error) -> Error {
  if matches!(err, ureq::Error::Timeout(_)) {
    return Error::Timeout;
  }
  if let ureq::Error::Io(io) = err {
    return map_io(io);
  }
  Error::Transport(err.to_string())
}

fn map_io(err: &std::io::Error) -> Error {
  if err.kind() == std::io::ErrorKind::TimedOut {
    Error::Timeout
  } else {
    Error::Transport(err.to_string())
  }
}

/// Test-only rustls types for the reusable in-process fixture crate.
#[cfg(feature = "test-support")]
pub use rustls as fixture_rustls;

#[cfg(test)]
#[path = "tests/lib_test.rs"]
mod tests;
