//! Shared `OpenRouter` transport for structured chat calls. Credentials come only
//! from the supplied environment; later clients can add streaming and tool use.

use std::fmt;
use std::time::Duration;

use toolu_runtime::env::Env;

mod error;
mod request;
mod response;
mod retry;

pub use error::Error;
pub use request::{Message, StructuredRequest};
pub use response::{Completion, Usage};

/// The non-streaming chat endpoint.
pub const ENDPOINT: &str = "https://openrouter.ai/api/v1/chat/completions";
/// Pinned default review model.
pub const DEFAULT_MODEL: &str = "deepseek/deepseek-v4-pro-0813";
/// Bounded response body, matching the code-review budget.
pub const MAX_BODY_BYTES: usize = 1024 * 1024;

/// This crate's layer in `tooling/conventions/guardrails/rust/layers.json`.
pub const LAYER: &str = "openrouter";

/// Endpoint and transport settings. Only tests override the endpoint and CA.
#[derive(Debug, Clone)]
pub struct Config {
  /// HTTPS endpoint.
  pub endpoint: String,
  /// Unit of retry delay; defaults to one second.
  pub pause: Duration,
  /// Loopback test root, when a fixture provides one.
  pub test_root_ca_der: Option<Vec<u8>>,
}

impl Default for Config {
  fn default() -> Self {
    Self {
      endpoint: ENDPOINT.to_owned(),
      pause: Duration::from_secs(1),
      test_root_ca_der: None,
    }
  }
}

/// A client whose key is never printed or exposed.
pub struct Client {
  key: String,
  model: String,
  config: Config,
  http: toolu_http::Client,
}

impl fmt::Debug for Client {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("Client")
      .field("endpoint", &self.config.endpoint)
      .field("model", &self.model)
      .finish_non_exhaustive()
  }
}

impl Client {
  /// Read `OPENROUTER_API_KEY` and optional `OPENROUTER_MODEL` from `env`.
  ///
  /// # Errors
  /// Refuses absent or malformed credentials and an unusable endpoint before
  /// any request.
  pub fn from_env(env: &Env, config: Config) -> Result<Self, Error> {
    let key = env
      .get("OPENROUTER_API_KEY")
      .filter(|key| !key.trim().is_empty())
      .ok_or(Error::Unconfigured)?;
    // A control character cannot travel in a header; refuse it before any retry.
    if key.contains(char::is_control) {
      return Err(Error::InvalidKey);
    }
    // The key travels as a bearer header, so a plaintext endpoint is refused.
    let https = config
      .endpoint
      .get(..8)
      .is_some_and(|scheme| scheme.eq_ignore_ascii_case("https://"));
    if !https || toolu_http::check_url(&config.endpoint).is_err() {
      return Err(Error::InvalidConfig);
    }
    // An empty override, as CI renders an unset variable, keeps the pinned default.
    let model = env
      .get("OPENROUTER_MODEL")
      .filter(|model| !model.is_empty())
      .unwrap_or(DEFAULT_MODEL);
    if model.trim().is_empty() || model.contains(['\r', '\n']) {
      return Err(Error::InvalidConfig);
    }
    let http = toolu_http::Client::new(
      toolu_http::Config {
        timeout: Duration::from_secs(30),
        max_body_bytes: MAX_BODY_BYTES,
        max_redirects: 0,
        test_root_ca_der: config.test_root_ca_der.clone(),
      },
      env,
    )
    .map_err(|_http| Error::InvalidConfig)?;
    Ok(Self {
      key: key.trim().to_owned(),
      model: model.to_owned(),
      config,
      http,
    })
  }

  /// The model requested by every call, including an environment override.
  pub fn model(&self) -> &str {
    &self.model
  }
}

#[cfg(test)]
#[path = "tests/lib_test.rs"]
mod tests;
