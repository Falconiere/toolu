//! API retries for every tracker and GitHub call (`scripts/ratelimit.ts`).
//!
//! An epic run spends one API budget across the engine and every worker's
//! babysit, so calls back off instead of burning it: a transient failure
//! retries with exponential backoff, a rate limit sleeps until its reset when
//! that is within the cap, and a write never retries a transient failure that
//! may have applied. An engine effect also carries a total sleep budget, so
//! one effect cannot hold the effect thread past `EPIC_API_MAX_SLEEP_S`.

use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use toolu_runtime::env::Env;

mod map;
mod reset;
mod scope;

pub(crate) use map::{github_failure, http_failure};
pub(crate) use reset::reset_from_headers;
pub(crate) use scope::{effect_scope, scoped_budget};

/// How a failure is retried.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Class {
  /// Wait for the reset, or at least a minute when it is unknown.
  RateLimit,
  /// Back off and retry, unless the call is a write.
  Transient,
  /// Never retried.
  Permanent,
}

/// One failed attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Failure {
  /// What failed, with no credential in it.
  pub message: String,
  /// How to retry it.
  pub class: Class,
  /// When a rate limit resets, Unix milliseconds, when the reply said.
  pub reset_at_ms: Option<u64>,
}

/// Attempts, backoff base and the longest single wait.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Policy {
  /// Attempts, the first included.
  pub attempts: u32,
  /// The backoff base; attempt `n` waits `base * 2^n` plus jitter.
  pub base_ms: u64,
  /// The longest wait for a rate-limit reset before giving up.
  pub max_sleep_ms: u64,
}

impl Policy {
  /// `EPIC_API_ATTEMPTS` (5), `EPIC_API_BACKOFF_MS` (2000), `EPIC_API_MAX_SLEEP_S` (900).
  pub(crate) fn from_env(env: &Env) -> Self {
    Self {
      attempts: u32::try_from(number(env, "EPIC_API_ATTEMPTS", 5)).unwrap_or(5),
      base_ms: number(env, "EPIC_API_BACKOFF_MS", 2000),
      max_sleep_ms: number(env, "EPIC_API_MAX_SLEEP_S", 900).saturating_mul(1000),
    }
  }

  /// One engine effect's budget: all its sleeps together stay within the cap.
  pub(crate) fn effect(self) -> Budget {
    Budget {
      remaining_ms: self.max_sleep_ms,
    }
  }
}

/// A non-negative whole number from the environment, as `envNumber` reads it.
pub(crate) fn number(env: &Env, name: &str, fallback: u64) -> u64 {
  env
    .get(name)
    .and_then(|text| text.trim().parse::<f64>().ok())
    .filter(|value| value.is_finite() && *value >= 0.0)
    .map_or(fallback, |value| {
      let whole = value.floor();
      if whole >= 1.8e19 {
        u64::MAX
      } else {
        format!("{whole:.0}").parse().unwrap_or(fallback)
      }
    })
}

/// Sleep that is still allowed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Budget {
  /// Milliseconds of sleep left.
  pub remaining_ms: u64,
}

impl Budget {
  /// A budget limited only by the per-wait cap, for one CLI call.
  pub(crate) fn unbounded() -> Self {
    Self {
      remaining_ms: u64::MAX,
    }
  }
}

/// Run `call` under `policy`, spending `budget`. `write` marks a call that must
/// not repeat a transient failure.
///
/// # Errors
/// The last failure's message, or why waiting was refused.
pub(crate) fn with_retry<T>(
  policy: &Policy,
  budget: &mut Budget,
  write: bool,
  mut call: impl FnMut() -> Result<T, Failure>,
) -> Result<T, String> {
  let mut last = String::new();
  for attempt in 0..policy.attempts.max(1) {
    let failure = match call() {
      Ok(value) => return Ok(value),
      Err(failure) => failure,
    };
    let final_attempt = attempt.saturating_add(1) >= policy.attempts;
    if failure.class == Class::Permanent
      || (failure.class == Class::Transient && write)
      || final_attempt
    {
      return Err(failure.message);
    }
    let wait = wait_for(policy, attempt, &failure)?;
    if wait > budget.remaining_ms {
      return Err(format!("{}; the sleep budget is spent", failure.message));
    }
    budget.remaining_ms -= wait;
    thread::sleep(Duration::from_millis(wait));
    last = failure.message;
  }
  Err(last)
}

fn wait_for(policy: &Policy, attempt: u32, failure: &Failure) -> Result<u64, String> {
  let backoff = policy
    .base_ms
    .saturating_mul(1_u64 << attempt.min(30))
    .saturating_add(jitter(policy.base_ms));
  let wait = match (failure.class, failure.reset_at_ms) {
    (Class::RateLimit, Some(reset)) => {
      let wait = backoff.max(reset.saturating_sub(now_ms()).saturating_add(1000));
      if wait > policy.max_sleep_ms {
        return Err(format!(
          "rate limited until {}; longer than the {}s cap",
          toolu_state::time::iso_millis(UNIX_EPOCH + Duration::from_millis(reset)),
          policy.max_sleep_ms / 1000
        ));
      }
      wait
    }
    (Class::RateLimit, None) => backoff.max(60_000),
    (Class::Transient | Class::Permanent, _) => backoff,
  };
  Ok(wait.min(policy.max_sleep_ms))
}

fn jitter(base_ms: u64) -> u64 {
  if base_ms == 0 {
    return 0;
  }
  let nanos = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .map_or(0, |since| u64::from(since.subsec_nanos()));
  nanos % base_ms
}

/// The clock in Unix milliseconds.
pub(crate) fn now_ms() -> u64 {
  toolu_state::time::epoch_millis(SystemTime::now())
}

/// Run `work` over `items` with at most `limit` in flight, keeping order. The
/// first failure is returned once every started item has finished.
///
/// # Errors
/// The first item's error, in item order.
pub(crate) fn map_limit<T: Sync, R: Send>(
  items: &[T],
  limit: usize,
  work: impl Fn(&T) -> Result<R, String> + Sync,
) -> Result<Vec<R>, String> {
  let mut out = Vec::with_capacity(items.len());
  for chunk in items.chunks(limit.max(1)) {
    let results: Vec<Result<R, String>> = thread::scope(|scope| {
      let handles: Vec<_> = chunk
        .iter()
        .map(|item| scope.spawn(|| work(item)))
        .collect();
      handles
        .into_iter()
        .map(|handle| {
          handle
            .join()
            .unwrap_or_else(|_| Err("a fetch thread panicked".to_owned()))
        })
        .collect()
    });
    for result in results {
      out.push(result?);
    }
  }
  Ok(out)
}

#[cfg(test)]
#[path = "tests/retry_test.rs"]
mod tests;
