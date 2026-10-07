//! `Error`: why a GitHub call failed. No message holds a token.

use std::fmt;
use std::time::Duration;

use crate::token::TokenError;

/// A GitHub call's failure.
#[derive(Debug, PartialEq, Eq)]
pub enum Error {
  /// No usable token.
  Token(TokenError),
  /// An invalid setting or path: a `PB_GH_*` value, the API URL, a retry
  /// policy, or a path outside the API.
  Config(String),
  /// GitHub answered `401` to the token and to a re-read one.
  Unauthorized,
  /// A rate limit whose wait is longer than the policy allows, or the last of
  /// the attempts.
  RateLimited {
    /// The HTTP status, 403 or 429.
    status: u16,
    /// How long GitHub asked to wait, when it said.
    retry_after: Option<Duration>,
  },
  /// A permanent HTTP error, or the last of the attempts.
  Status {
    /// The HTTP status.
    status: u16,
    /// The JSON body's `message`, redacted, or empty.
    message: String,
  },
  /// A GraphQL reply with `errors[]`: their messages, redacted.
  GraphQl(Vec<String>),
  /// The transport failed on the last attempt, or permanently.
  Transport(toolu_http::Error),
  /// A reply that could not be decoded.
  Decode(String),
}

impl fmt::Display for Error {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Token(err) => err.fmt(f),
      Self::Config(reason) => write!(f, "GitHub client configuration: {reason}"),
      Self::Unauthorized => {
        f.write_str("GitHub rejected the token (401), also after re-reading it")
      }
      Self::RateLimited {
        status,
        retry_after: Some(wait),
      } => write!(
        f,
        "GitHub rate limit (HTTP {status}): retry in {}s",
        wait.as_secs()
      ),
      Self::RateLimited { status, .. } => write!(f, "GitHub rate limit (HTTP {status})"),
      Self::Status { status, message } if message.is_empty() => {
        write!(f, "GitHub answered HTTP {status}")
      }
      Self::Status { status, message } => write!(f, "GitHub answered HTTP {status}: {message}"),
      Self::GraphQl(messages) => write!(f, "GitHub GraphQL errors: {}", messages.join("; ")),
      Self::Transport(err) => write!(f, "GitHub request failed: {err}"),
      Self::Decode(reason) => write!(f, "cannot decode the GitHub reply: {reason}"),
    }
  }
}

impl std::error::Error for Error {}

/// A JSON decoding failure, described by its kind and place only: serde's own
/// message may quote the value it rejected.
pub(crate) fn decode(err: &serde_json::Error) -> Error {
  let kind = match err.classify() {
    serde_json::error::Category::Io => "I/O",
    serde_json::error::Category::Syntax => "syntax",
    serde_json::error::Category::Data => "unexpected data",
    serde_json::error::Category::Eof => "end of input",
  };
  Error::Decode(format!(
    "{kind} error at line {} column {}",
    err.line(),
    err.column()
  ))
}

impl From<TokenError> for Error {
  fn from(err: TokenError) -> Error {
    Error::Token(err)
  }
}

#[cfg(test)]
#[path = "tests/error_test.rs"]
mod tests;
