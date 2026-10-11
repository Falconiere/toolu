//! `Cost`: what a call spent of GitHub's rate limits, for the engine's budget
//! and journal.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use toolu_http::Response;

/// The `x-ratelimit-*` headers of a response.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RateLimit {
  /// `x-ratelimit-resource`: `core`, `graphql`, …
  pub resource: Option<String>,
  /// `x-ratelimit-limit`.
  pub limit: Option<u64>,
  /// `x-ratelimit-remaining`.
  pub remaining: Option<u64>,
  /// `x-ratelimit-used`.
  pub used: Option<u64>,
  /// `x-ratelimit-reset`, Unix seconds.
  pub reset: Option<u64>,
}

impl RateLimit {
  /// Read the primary rate-limit counters from response headers.
  pub(crate) fn of(response: &Response) -> RateLimit {
    let number = |name: &str| {
      response
        .header(name)
        .and_then(|value| value.trim().parse().ok())
    };
    RateLimit {
      resource: response.header("x-ratelimit-resource").map(str::to_owned),
      limit: number("x-ratelimit-limit"),
      remaining: number("x-ratelimit-remaining"),
      used: number("x-ratelimit-used"),
      reset: number("x-ratelimit-reset"),
    }
  }
}

/// The cost of a call's final response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Cost {
  /// Rate-limit points: REST 0 for a `304`, else 1; GraphQL the
  /// `rateLimit.cost` the query selected, if it did.
  pub points: Option<u64>,
  /// The response's rate-limit headers.
  pub rate: RateLimit,
}

impl Cost {
  /// A REST response: a `304` costs nothing (GitHub documents this).
  pub(crate) fn rest(response: &Response) -> Cost {
    Cost {
      points: Some(u64::from(response.status != 304)),
      rate: RateLimit::of(response),
    }
  }

  /// A GraphQL response whose `data` may hold `rateLimit { cost }`.
  pub(crate) fn graphql(response: &Response, data: &Value) -> Cost {
    Cost {
      points: data.pointer("/rateLimit/cost").and_then(Value::as_u64),
      rate: RateLimit::of(response),
    }
  }
}

#[cfg(test)]
#[path = "tests/cost_test.rs"]
mod tests;
