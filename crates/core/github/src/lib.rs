//! toolu core, `github` layer: the GitHub REST and GraphQL client that
//! pr-babysit, the epic engine's actions and its GitHub checks share (#460).
//!
//! - The token comes from `GH_TOKEN`, else `gh auth token`. It is read when the
//!   client starts and again on a `401`, and is never written or printed.
//! - REST calls are conditional on the caller's `ETag`; GraphQL `errors[]` are
//!   errors. Every reply reports its rate-limit [`Cost`].
//! - [`Config::one_shot`] keeps pr-babysit's gh retries for one-shot CLI calls;
//!   [`Config::scheduled`] makes one attempt for the engine's fixed schedule.

use std::fmt;
use std::sync::Mutex;
use std::time::Duration;

use toolu_runtime::env::Env;

mod call;
mod cost;
mod error;
mod graphql;
mod policy;
mod rest;
mod token;

pub use cost::{Cost, RateLimit};
pub use error::Error;
pub use policy::Retry;
pub use rest::{Fresh, Rest, RestRequest};
pub use token::{Source, TokenError};
pub use toolu_http::Method;

/// This crate's layer in `tooling/conventions/guardrails/rust/layers.json`.
pub const LAYER: &str = "github";

/// The GitHub API.
pub const API_URL: &str = "https://api.github.com";

/// The largest reply read: GraphQL review-thread replies outgrow toolu-http's
/// 1 MiB default.
const MAX_BODY_BYTES: usize = 8 * 1024 * 1024;

/// What a call produced, what it cost, and how many attempts it took.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply<T> {
  /// The reply.
  pub data: T,
  /// The cost of the final response.
  pub cost: Cost,
  /// Attempts, not counting a retry after a re-read token.
  pub attempts: u32,
}

/// A client's API, retry policy and transport settings.
#[derive(Debug, Clone)]
pub struct Config {
  /// The API's `https://` URL, without a trailing `/`.
  pub api_url: String,
  /// When a call is tried again.
  pub retry: Retry,
  /// The transport's deadline, body cap and test root.
  pub http: toolu_http::Config,
}

impl Config {
  /// pr-babysit's one-shot gh semantics: `PB_GH_ATTEMPTS` (3), `PB_GH_BACKOFF`
  /// ("2 4 8") and `PB_GH_TIMEOUT` (60 s per attempt).
  ///
  /// # Errors
  /// `Config` naming the variable whose value is invalid.
  pub fn one_shot(env: &Env) -> Result<Config, Error> {
    let (retry, timeout) = policy::one_shot(env)?;
    Ok(Config {
      api_url: API_URL.to_owned(),
      retry,
      http: http_config(timeout),
    })
  }

  /// The engine's checks: one attempt, a rate limit returned with its wait.
  pub fn scheduled() -> Config {
    Config {
      api_url: API_URL.to_owned(),
      retry: Retry::scheduled(),
      http: http_config(Duration::from_secs(30)),
    }
  }
}

fn http_config(timeout: Duration) -> toolu_http::Config {
  toolu_http::Config {
    timeout,
    max_body_bytes: MAX_BODY_BYTES,
    ..toolu_http::Config::default()
  }
}

/// A GitHub client. Its `Debug` shows the API and the retry policy only.
pub struct Client {
  http: toolu_http::Client,
  env: Env,
  config: Config,
  tokens: Mutex<token::Tokens>,
}

impl fmt::Debug for Client {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("Client")
      .field("api_url", &self.config.api_url)
      .field("retry", &self.config.retry)
      .finish_non_exhaustive()
  }
}

impl Client {
  /// Check `config`, then read the token from `env`.
  ///
  /// # Errors
  /// `Config` for an API URL that is not `https://` or a policy without
  /// attempts; `Token` when no token can be read; `Transport` for a transport
  /// setting toolu-http refuses.
  pub fn new(mut config: Config, env: &Env) -> Result<Client, Error> {
    let trimmed = config.api_url.trim_end_matches('/').len();
    config.api_url.truncate(trimmed);
    let host = config.api_url.strip_prefix("https://").unwrap_or_default();
    if host.is_empty() || host.contains(['/', '?', '#']) {
      return Err(Error::Config(format!(
        "the API URL must be an https:// origin, not {}",
        config.api_url
      )));
    }
    if config.retry.attempts == 0 {
      return Err(Error::Config(
        "the retry policy needs at least 1 attempt".into(),
      ));
    }
    let http = toolu_http::Client::new(config.http.clone(), env).map_err(Error::Transport)?;
    let token = token::resolve(env)?;
    Ok(Client {
      http,
      env: env.clone(),
      config,
      tokens: Mutex::new(token::Tokens::new(token)),
    })
  }
}

#[cfg(test)]
#[path = "tests/lib_test.rs"]
mod tests;
