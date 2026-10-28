//! toolu core, `jev` layer: the Jev HTTP client (#460) that `toolu jev` (#430),
//! pr-babysit (#433) and the epic engine (#435) share.
//!
//! - The key comes from `TYPESAFE_API_KEY` and is never printed; `from_env_key`
//!   names another variable, as code-review does for OpenRouter's System One.
//! - `JEV_TIMEOUT` is the per-attempt deadline in seconds (60 by default).
//! - A call makes up to three attempts, as `plugins/jev/hooks/src/jev.ts` does;
//!   `ask_until` also bounds every attempt and pause by one wall deadline.
//! - State and questions keep their JSON key order, so a request carries the
//!   text `jev.ts` sends. The reply is checked into typed answers.

use std::fmt;
use std::time::{Duration, Instant};

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

/// The same System One request body, served by OpenRouter with its key.
pub const OPENROUTER_ENDPOINT: &str = "https://openrouter.ai/api/v1/systemone";

/// The Jev model pinned on OpenRouter.
pub const OPENROUTER_MODEL: &str = "typesafe/jev-1.13";

/// The key variable `from_env` reads.
const KEY_VAR: &str = "TYPESAFE_API_KEY";

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

/// The API key. It has no `Debug`, so nothing can print it.
struct Key(String);

/// A Jev client. Its `Debug` shows the endpoint and timeout only.
pub struct Jev {
  /// `None` when `JEV_TIMEOUT` is 0: every attempt times out unsent.
  http: Option<toolu_http::Client>,
  /// What a shorter, deadline-bound attempt client is built from.
  env: Env,
  root: Option<Vec<u8>>,
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
  /// `Transport` for a transport setting toolu-http refuses, such as a
  /// `NODE_EXTRA_CA_CERTS` file that is missing or holds no certificate.
  pub fn from_env(env: &Env, config: Config) -> Result<Jev, Error> {
    Jev::from_env_key(env, KEY_VAR, config)
  }

  /// `from_env` with the key read from `key_var` instead of `TYPESAFE_API_KEY`.
  ///
  /// # Errors
  /// As `from_env`, for the named variable.
  pub fn from_env_key(env: &Env, key_var: &str, config: Config) -> Result<Jev, Error> {
    let key = env.get(key_var).ok_or(Error::MissingKey)?;
    if key.contains(['\r', '\n']) {
      return Err(Error::KeyLineBreak);
    }
    let timeout = timeout(env.get("JEV_TIMEOUT"));
    let root = trusted_root(env, config.test_root_ca_der)?;
    let http = if timeout.is_zero() {
      None
    } else {
      Some(http_client(env, timeout, root.clone())?)
    };
    Ok(Jev {
      http,
      env: env.clone(),
      root,
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
    let text = self.post(body(state, model, questions).as_bytes(), None)?;
    reply::parse(&text, questions)
  }

  /// `ask` within one wall `deadline` shared by every attempt and pause: an
  /// attempt never outlives it, and a pause that would pass it ends the call.
  ///
  /// # Errors
  /// As `ask`; `Timeout` once the deadline has passed, or the last failure
  /// when the next pause would pass it.
  pub fn ask_until(
    &self,
    state: &State,
    model: &str,
    questions: &Questions,
    deadline: Instant,
  ) -> Result<Reply, Error> {
    let text = self.post(body(state, model, questions).as_bytes(), Some(deadline))?;
    reply::parse(&text, questions)
  }
}

/// The exact byte length of the request body `ask` sends, so callers can hold
/// a request cap before any network call.
pub fn request_bytes(state: &State, model: &str, questions: &Questions) -> usize {
  body(state, model, questions).len()
}

fn body(state: &State, model: &str, questions: &Questions) -> String {
  Ordered::Object(vec![
    ("state".to_owned(), state.value().clone()),
    ("model".to_owned(), Ordered::String(model.to_owned())),
    ("questions".to_owned(), questions.value()),
  ])
  .to_text(false)
}

/// An HTTP client with `timeout` per attempt and no redirects.
fn http_client(
  env: &Env,
  timeout: Duration,
  root: Option<Vec<u8>>,
) -> Result<toolu_http::Client, Error> {
  let http = toolu_http::Config {
    timeout,
    max_redirects: 0,
    test_root_ca_der: root,
    ..toolu_http::Config::default()
  };
  toolu_http::Client::new(http, env).map_err(Error::Transport)
}

/// The one root the client trusts: the loopback test root, else the first
/// certificate of the PEM file `NODE_EXTRA_CA_CERTS` names, else none (the
/// webpki roots). Not an endpoint override.
fn trusted_root(env: &Env, test_root: Option<Vec<u8>>) -> Result<Option<Vec<u8>>, Error> {
  if test_root.is_some() {
    return Ok(test_root);
  }
  env
    .get("NODE_EXTRA_CA_CERTS")
    .map(toolu_http::pem_file_to_der)
    .transpose()
    .map_err(Error::Transport)
}

/// `JEV_TIMEOUT` seconds; 60 when unset, negative, not a number, or over
/// `u32::MAX` seconds, past which a deadline overflows the clock.
fn timeout(value: Option<&str>) -> Duration {
  let parsed = value
    .and_then(|text| text.trim().parse::<f64>().ok())
    .filter(|seconds| (0.0..=f64::from(u32::MAX)).contains(seconds))
    .and_then(|seconds| Duration::try_from_secs_f64(seconds).ok());
  let Some(timeout) = parsed else {
    return DEFAULT_TIMEOUT;
  };
  timeout
}

#[cfg(test)]
#[path = "tests/lib_test.rs"]
mod tests;
