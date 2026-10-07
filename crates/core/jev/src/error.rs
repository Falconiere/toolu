//! `Error`: why a Jev call gave no judgment. No message holds the key.

use std::fmt;

/// A Jev call's failure.
#[derive(Debug, PartialEq, Eq)]
pub enum Error {
  /// `TYPESAFE_API_KEY` is unset or empty.
  MissingKey,
  /// `TYPESAFE_API_KEY` holds a line break.
  KeyLineBreak,
  /// A question or `ask` payload refused before any request.
  InvalidQuestion(String),
  /// A final non-2xx answer and its body, with the key redacted.
  Http {
    /// The HTTP status.
    status: u16,
    /// The response body.
    body: String,
  },
  /// The last attempt timed out.
  Timeout,
  /// The transport failed on the last attempt, or the body was over the cap.
  Transport(toolu_http::Error),
  /// The reply is not a typed answer for every question.
  InvalidResponse,
}

impl fmt::Display for Error {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::MissingKey => f.write_str("TYPESAFE_API_KEY unset"),
      Self::KeyLineBreak => f.write_str("TYPESAFE_API_KEY must not contain line breaks"),
      Self::InvalidQuestion(reason) => f.write_str(reason),
      Self::Http { status, body } => write!(f, "Jev answered HTTP {status}: {body}"),
      Self::Timeout => f.write_str("the Jev request timed out"),
      Self::Transport(err) => write!(f, "the Jev request failed: {err}"),
      Self::InvalidResponse => {
        f.write_str("invalid response: expected a typed answer for every question")
      }
    }
  }
}

impl std::error::Error for Error {}

#[cfg(test)]
#[path = "tests/error_test.rs"]
mod tests;
