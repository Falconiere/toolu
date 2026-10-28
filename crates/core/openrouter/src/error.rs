//! Typed failures without upstream bodies, prompt text or credentials.

use std::fmt;

use crate::Usage;

/// Why a structured completion could not be used.
#[derive(Debug, Clone, PartialEq)]
pub enum Error {
  /// No `OPENROUTER_API_KEY` in the supplied environment.
  Unconfigured,
  /// Credential contains a line break or another control character.
  InvalidKey,
  /// Endpoint, model or request is unusable.
  InvalidConfig,
  /// Authentication or authorization failed.
  Authentication(u16),
  /// `OpenRouter` limited the request.
  RateLimited {
    /// Whole seconds supplied by `Retry-After`, if any.
    retry_after_secs: Option<u64>,
  },
  /// No endpoint met the routing constraints.
  NoProvider,
  /// A provider or `OpenRouter` returned a non-success status.
  Upstream(u16),
  /// The shared wall deadline elapsed.
  Timeout,
  /// TLS, proxy or connection failed.
  Transport,
  /// The response exceeded the byte cap.
  BodyTooLarge,
  /// The response envelope lacked required fields or had invalid JSON.
  InvalidResponse,
  /// No visible content; the finish reason is kept for diagnosis.
  Empty {
    /// `OpenRouter`'s finish reason, if supplied.
    finish_reason: Option<String>,
    /// Billed usage, when the response includes it.
    usage: Option<Usage>,
  },
  /// Reasoning consumed the completion budget.
  Truncated {
    /// `OpenRouter`'s finish reason, if supplied.
    finish_reason: Option<String>,
    /// Billed usage, when the response includes it.
    usage: Option<Usage>,
  },
}

impl fmt::Display for Error {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Unconfigured => f.write_str("OPENROUTER_API_KEY is missing"),
      Self::InvalidKey => f.write_str("OPENROUTER_API_KEY contains a control character"),
      Self::InvalidConfig => f.write_str("invalid OpenRouter configuration or request"),
      Self::Authentication(status) => write!(f, "OpenRouter authentication failed (HTTP {status})"),
      Self::RateLimited { retry_after_secs } => write!(
        f,
        "OpenRouter rate limited the request (retry-after: {retry_after_secs:?})"
      ),
      Self::NoProvider => f.write_str("no OpenRouter provider met the routing constraints"),
      Self::Upstream(status) => write!(f, "OpenRouter or provider failed (HTTP {status})"),
      Self::Timeout => f.write_str("OpenRouter call exceeded its deadline"),
      Self::Transport => f.write_str("OpenRouter transport failed"),
      Self::BodyTooLarge => f.write_str("OpenRouter response exceeded its byte limit"),
      Self::InvalidResponse => f.write_str("OpenRouter returned an invalid response envelope"),
      Self::Empty { finish_reason, .. } => write!(
        f,
        "OpenRouter returned no visible content (finish_reason: {finish_reason:?})"
      ),
      Self::Truncated { finish_reason, .. } => write!(
        f,
        "OpenRouter reasoning exhausted the completion budget (finish_reason: {finish_reason:?})"
      ),
    }
  }
}

impl std::error::Error for Error {}

impl Error {
  /// Usage reported on an empty or truncated billed completion, if any.
  pub fn usage(&self) -> Option<&Usage> {
    match self {
      Self::Empty { usage, .. } | Self::Truncated { usage, .. } => usage.as_ref(),
      Self::Unconfigured
      | Self::InvalidKey
      | Self::InvalidConfig
      | Self::Authentication(_)
      | Self::RateLimited { .. }
      | Self::NoProvider
      | Self::Upstream(_)
      | Self::Timeout
      | Self::Transport
      | Self::BodyTooLarge
      | Self::InvalidResponse => None,
    }
  }

  /// The typed failure for an HTTP or embedded error `code` and its message.
  pub(crate) fn from_status(
    code: u16,
    message: Option<&str>,
    retry_after_secs: Option<u64>,
  ) -> Self {
    if message.is_some_and(|message| {
      let message = message.to_ascii_lowercase();
      message.contains("no endpoints") || message.contains("no provider")
    }) {
      return Self::NoProvider;
    }
    match code {
      401 | 403 => Self::Authentication(code),
      429 => Self::RateLimited { retry_after_secs },
      _ => Self::Upstream(code),
    }
  }
}

#[cfg(test)]
#[path = "tests/error_test.rs"]
mod tests;
