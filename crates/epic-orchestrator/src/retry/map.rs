//! Turn a `toolu-github` error or a `toolu-http` reply into a [`Failure`]: a
//! rate limit with its reset, a transient 5xx or transport fault, or a
//! permanent error.

use std::time::Duration;

use super::{Class, Failure, now_ms, reset_from_headers};

/// The retry class of a GitHub client error.
pub(crate) fn github_failure(err: &toolu_github::Error) -> Failure {
  let message = err.to_string();
  match err {
    toolu_github::Error::RateLimited {
      retry_after, rate, ..
    } => Failure {
      message,
      class: Class::RateLimit,
      reset_at_ms: github_reset(*retry_after, rate),
    },
    toolu_github::Error::Status {
      status: 500 | 502 | 503 | 504,
      ..
    }
    | toolu_github::Error::Transport(_) => Failure {
      message,
      class: Class::Transient,
      reset_at_ms: None,
    },
    toolu_github::Error::Token(_)
    | toolu_github::Error::Config(_)
    | toolu_github::Error::Unauthorized
    | toolu_github::Error::Status { .. }
    | toolu_github::Error::GraphQl(_)
    | toolu_github::Error::Decode(_) => Failure {
      message,
      class: Class::Permanent,
      reset_at_ms: None,
    },
  }
}

fn github_reset(retry_after: Option<Duration>, rate: &toolu_github::RateLimit) -> Option<u64> {
  if let Some(wait) = retry_after {
    let millis = u64::try_from(wait.as_millis()).unwrap_or(u64::MAX);
    return Some(now_ms().saturating_add(millis));
  }
  if rate.remaining == Some(0) {
    return rate.reset.map(|reset| reset.saturating_mul(1000));
  }
  None
}

/// The retry class of one HTTP exchange: `Ok(body)` for a 2xx reply, the
/// failure otherwise. `label` names the service in the message.
///
/// # Errors
/// A non-2xx status, a GraphQL `RATELIMITED` error, or a transport failure.
pub(crate) fn http_failure(
  label: &str,
  url: &str,
  outcome: Result<toolu_http::Response, toolu_http::Error>,
) -> Result<Vec<u8>, Failure> {
  let response = match outcome {
    Ok(response) => response,
    Err(err) => return Err(transport(label, url, &err)),
  };
  let status = response.status;
  let text = String::from_utf8_lossy(&response.body);
  if (200..300).contains(&status) {
    if ratelimited_graphql(&text) {
      return Err(Failure {
        message: format!("{label}: RATELIMITED {url}"),
        class: Class::RateLimit,
        reset_at_ms: reset_from_headers(|name| response.header(name), now_ms()),
      });
    }
    return Ok(response.body);
  }
  let snippet: String = text.chars().take(300).collect();
  let message = format!("{label}: HTTP {status} {url}: {snippet}");
  // The status decides, never the URL or body text: a key such as `PAY-503`
  // in a 404 stays permanent. A GraphQL `RATELIMITED` error is a rate limit
  // whatever its status.
  let class = match status {
    429 => Class::RateLimit,
    500 | 502 | 503 | 504 => Class::Transient,
    _ if ratelimited_graphql(&text) => Class::RateLimit,
    _ => Class::Permanent,
  };
  Err(Failure {
    message,
    class,
    reset_at_ms: reset_from_headers(|name| response.header(name), now_ms()),
  })
}

fn transport(label: &str, url: &str, err: &toolu_http::Error) -> Failure {
  let class = match err {
    toolu_http::Error::Timeout | toolu_http::Error::Transport(_) => Class::Transient,
    toolu_http::Error::InvalidConfig(_)
    | toolu_http::Error::HttpStatus(_)
    | toolu_http::Error::BodyTooLarge
    | toolu_http::Error::Encode(_)
    | toolu_http::Error::Decode(_) => Class::Permanent,
  };
  Failure {
    message: format!("{label}: {err} {url}"),
    class,
    reset_at_ms: None,
  }
}

fn ratelimited_graphql(text: &str) -> bool {
  serde_json::from_str::<serde_json::Value>(text)
    .ok()
    .and_then(|body| {
      body
        .get("errors")
        .and_then(serde_json::Value::as_array)
        .cloned()
    })
    .is_some_and(|errors| {
      errors.iter().any(|error| {
        error
          .pointer("/extensions/code")
          .and_then(serde_json::Value::as_str)
          == Some("RATELIMITED")
      })
    })
}

#[cfg(test)]
#[path = "tests/map_test.rs"]
mod tests;
