//! When a call is tried again. The one-shot policy keeps pr-babysit's gh
//! semantics (`PB_GH_ATTEMPTS`, `PB_GH_BACKOFF`, `PB_GH_TIMEOUT`,
//! `plugins/pr-babysit/hooks/src/babysit/gh.ts`); the scheduled one serves the
//! engine's fixed 3-minute checks and never waits.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use toolu_http::Response;
use toolu_runtime::env::Env;

use crate::Error;

/// The wait when the backoff list has no entry for an attempt (`gh.ts`).
const FALLBACK_BACKOFF: Duration = Duration::from_secs(2);

/// The longest rate-limit wait a one-shot call sleeps through.
const ONE_SHOT_MAX_WAIT: Duration = Duration::from_secs(60);

/// How often a call is tried, and how long it waits in between.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Retry {
  /// Attempts, at least 1.
  pub attempts: u32,
  /// The wait after each failed attempt, in order.
  pub backoff: Vec<Duration>,
  /// A rate-limit wait longer than this returns `RateLimited` at once.
  pub max_wait: Duration,
}

impl Retry {
  /// One attempt; a rate limit is returned with its wait, never slept.
  pub fn scheduled() -> Retry {
    Retry {
      attempts: 1,
      backoff: Vec::new(),
      max_wait: Duration::ZERO,
    }
  }

  /// The wait after failed attempt `attempt` (1-based).
  pub(crate) fn backoff(&self, attempt: u32) -> Duration {
    let index = usize::try_from(attempt.saturating_sub(1)).unwrap_or(usize::MAX);
    self.backoff.get(index).copied().unwrap_or(FALLBACK_BACKOFF)
  }
}

/// The one-shot policy and per-attempt timeout from `PB_GH_ATTEMPTS` (3),
/// `PB_GH_BACKOFF` ("2 4 8") and `PB_GH_TIMEOUT` (60 seconds).
pub(crate) fn one_shot(env: &Env) -> Result<(Retry, Duration), Error> {
  let attempts = match env.get("PB_GH_ATTEMPTS") {
    None => 3,
    Some(text) => text
      .parse::<u32>()
      .ok()
      .filter(|attempts| *attempts > 0)
      .ok_or_else(|| Error::Config("PB_GH_ATTEMPTS must be a positive integer".into()))?,
  };
  let backoff = env
    .get("PB_GH_BACKOFF")
    .unwrap_or("2 4 8")
    .split_whitespace()
    .map(|word| seconds(word, false))
    .collect::<Option<Vec<_>>>()
    .ok_or_else(|| {
      Error::Config("PB_GH_BACKOFF must list non-negative numbers of seconds".into())
    })?;
  let timeout = seconds(env.get("PB_GH_TIMEOUT").unwrap_or("60"), true)
    .ok_or_else(|| Error::Config("PB_GH_TIMEOUT must be a positive number of seconds".into()))?;
  let retry = Retry {
    attempts,
    backoff,
    max_wait: ONE_SHOT_MAX_WAIT,
  };
  Ok((retry, timeout))
}

/// `word` as a duration in seconds: non-negative, or positive when `positive`.
fn seconds(word: &str, positive: bool) -> Option<Duration> {
  let value = word.parse::<f64>().ok()?;
  if !value.is_finite() || value < 0.0 || (positive && value == 0.0) {
    return None;
  }
  Duration::try_from_secs_f64(value).ok()
}

/// What a response, or a failed request, means for the call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Class {
  /// 2xx or 304.
  Success,
  /// 401: re-read the token once.
  Unauthorized,
  /// A rate limit; `wait` when GitHub said how long.
  RateLimited {
    /// `retry-after`, or the reset of an exhausted limit.
    wait: Option<Duration>,
  },
  /// Worth another attempt after the backoff.
  Transient,
  /// Never worth another attempt.
  Permanent,
}

/// Classify one attempt as `gh.ts`'s `ghClassify` does for gh's statuses.
pub(crate) fn classify(outcome: &Result<Response, toolu_http::Error>) -> Class {
  let response = match outcome {
    Ok(response) => response,
    Err(toolu_http::Error::Timeout | toolu_http::Error::Transport(_)) => return Class::Transient,
    Err(_) => return Class::Permanent,
  };
  match response.status {
    200..=299 | 304 => Class::Success,
    401 => Class::Unauthorized,
    403 | 429 => match rate_wait(response, now()) {
      Some(wait) => Class::RateLimited { wait: Some(wait) },
      None if response.status == 429 || mentions_rate_limit(response) => {
        Class::RateLimited { wait: None }
      }
      None => Class::Permanent,
    },
    500..=599 => Class::Transient,
    _ => Class::Permanent,
  }
}

/// GitHub's wait: an integer `retry-after`, else the reset of an exhausted
/// limit (`x-ratelimit-remaining: 0`), relative to `now` (Unix seconds).
pub(crate) fn rate_wait(response: &Response, now: u64) -> Option<Duration> {
  if let Some(seconds) = response
    .header("retry-after")
    .and_then(|value| value.trim().parse::<u64>().ok())
  {
    return Some(Duration::from_secs(seconds));
  }
  if response.header("x-ratelimit-remaining").map(str::trim) != Some("0") {
    return None;
  }
  let reset = response
    .header("x-ratelimit-reset")?
    .trim()
    .parse::<u64>()
    .ok()?;
  Some(Duration::from_secs(reset.saturating_sub(now)))
}

fn mentions_rate_limit(response: &Response) -> bool {
  String::from_utf8_lossy(&response.body)
    .to_ascii_lowercase()
    .contains("rate limit")
}

fn now() -> u64 {
  SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .map_or(0, |elapsed| elapsed.as_secs())
}

#[cfg(test)]
#[path = "tests/policy_test.rs"]
mod tests;
