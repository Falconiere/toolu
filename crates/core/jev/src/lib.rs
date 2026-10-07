//! toolu core, `jev` layer: the Jev HTTP client (#460) that `toolu jev` (#430),
//! pr-babysit (#433) and the epic engine (#435) share.
//!
//! - The key comes from `TYPESAFE_API_KEY` and is never printed.
//! - `JEV_TIMEOUT` is the per-attempt deadline in seconds (60 by default).
//! - A call makes up to three attempts, as `plugins/jev/hooks/src/jev.ts` does.
//! - State and questions keep their JSON key order, so a request carries the
//!   text `jev.ts` sends. The reply is checked into typed answers.

use std::fmt;
use std::time::Duration;

use toolu_runtime::env::Env;
use toolu_runtime::json::ordered::Ordered;

mod error;
mod question;
mod reply;
mod retry;

pub use error::Error;
pub use question::{Question, Questions, State};
pub use reply::{Answer, Reply, Usage};

/// This crate's layer in `tooling/conventions/guardrails/rust/layers.json`.
pub const LAYER: &str = "jev";

/// Jev's endpoint.
pub const ENDPOINT: &str = "https://api.typesafe.ai/v1/systemone";

/// The model when the caller names none.
pub const DEFAULT_MODEL: &str = "jev-latest";

/// The per-attempt deadline without `JEV_TIMEOUT`.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);

/// Where requests go and how long a pause unit lasts.
#[derive(Debug, Clone)]
pub struct Config {
  /// The endpoint URL.
  pub endpoint: String,
  /// One pause unit between attempts (1 second; `retry-after` counts units).
  pub pause: Duration,
  /// DER-encoded root certificate used only by loopback tests.
  pub test_root_ca_der: Option<Vec<u8>>,
}

impl Default for Config {
  fn default() -> Self {
    Config {
      endpoint: ENDPOINT.to_owned(),
      pause: Duration::from_secs(1),
      test_root_ca_der: None,
    }
  }
}

/// The API key. Its `Debug` hides it.
struct Key(String);

/// A Jev client. Its `Debug` shows the endpoint and timeout only.
pub struct Jev {
  /// `None` when `JEV_TIMEOUT` is 0: every attempt times out unsent.
  http: Option<toolu_http::Client>,
  key: Key,
  endpoint: String,
  pause: Duration,
  timeout: Duration,
}

impl fmt::Debug for Jev {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("Jev")
      .field("endpoint", &self.endpoint)
      .field("timeout", &self.timeout)
      .finish_non_exhaustive()
  }
}

impl Jev {
  /// Read the key and timeout from `env`.
  ///
  /// # Errors
  /// `MissingKey` or `KeyLineBreak` for the key, before any request;
  /// `Transport` for a transport setting toolu-http refuses.
  pub fn from_env(env: &Env, config: Config) -> Result<Jev, Error> {
    let key = env.get("TYPESAFE_API_KEY").ok_or(Error::MissingKey)?;
    if key.contains(['\r', '\n']) {
      return Err(Error::KeyLineBreak);
    }
    let timeout = timeout(env.get("JEV_TIMEOUT"));
    let http = if timeout.is_zero() {
      None
    } else {
      let http = toolu_http::Config {
        timeout,
        max_redirects: 0,
        test_root_ca_der: config.test_root_ca_der,
        ..toolu_http::Config::default()
      };
      Some(toolu_http::Client::new(http, env).map_err(Error::Transport)?)
    };
    Ok(Jev {
      http,
      key: Key(key.to_owned()),
      endpoint: config.endpoint,
      pause: config.pause,
      timeout,
    })
  }

  /// Ask `questions` about `state` with `model`.
  ///
  /// # Errors
  /// `Http`, `Timeout` or `Transport` after the attempts, and
  /// `InvalidResponse` when the reply is not a typed answer for every question.
  pub fn ask(&self, state: &State, model: &str, questions: &Questions) -> Result<Reply, Error> {
    let body = Ordered::Object(vec![
      ("state".to_owned(), state.value().clone()),
      ("model".to_owned(), Ordered::String(model.to_owned())),
      ("questions".to_owned(), questions.value()),
    ])
    .to_text(false);
    let text = self.post(body.as_bytes())?;
    reply::parse(&text, questions)
  }
}

/// `JEV_TIMEOUT` seconds; 60 when unset, negative, not a number or too large.
fn timeout(value: Option<&str>) -> Duration {
  value
    .and_then(|text| text.trim().parse::<f64>().ok())
    .filter(|seconds| seconds.is_finite() && *seconds >= 0.0)
    .and_then(|seconds| Duration::try_from_secs_f64(seconds).ok())
    .unwrap_or(DEFAULT_TIMEOUT)
}

#[cfg(test)]
#[path = "tests/lib_test.rs"]
mod tests;
