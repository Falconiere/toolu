//! `Error`: what a request can fail with, and the mapping from ureq and I/O
//! failures into it.

use std::fmt;

/// A caller, server, timeout, size, JSON, or transport error.
#[derive(Debug, PartialEq, Eq)]
pub enum Error {
  /// An invalid client setting or request, such as a zero timeout, a malformed
  /// proxy or a credential header outside `Auth`.
  InvalidConfig(String),
  /// An HTTP 4xx or 5xx status.
  HttpStatus(u16),
  /// The whole-request deadline expired.
  Timeout,
  /// A response exceeded `Config::max_body_bytes`.
  BodyTooLarge,
  /// JSON request serialization failed.
  Encode(String),
  /// JSON response deserialization failed.
  Decode(String),
  /// A URL, TLS, proxy, redirect, or network error.
  Transport(String),
}

impl fmt::Display for Error {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::InvalidConfig(reason) => write!(f, "invalid HTTP configuration: {reason}"),
      Self::HttpStatus(status) => write!(f, "HTTP status {status}"),
      Self::Timeout => f.write_str("HTTP request timed out"),
      Self::BodyTooLarge => f.write_str("HTTP response body exceeds limit"),
      Self::Encode(reason) => write!(f, "cannot encode JSON request: {reason}"),
      Self::Decode(reason) => write!(f, "cannot decode JSON response: {reason}"),
      Self::Transport(reason) => write!(f, "HTTP transport error: {reason}"),
    }
  }
}

impl std::error::Error for Error {}

/// A request that could not be built.
pub(crate) fn map_http(err: &ureq::http::Error) -> Error {
  Error::Transport(err.to_string())
}

/// A failed ureq call: a deadline becomes `Timeout`, the rest `Transport`.
pub(crate) fn map_ureq(err: &ureq::Error) -> Error {
  if matches!(err, ureq::Error::Timeout(_)) {
    return Error::Timeout;
  }
  if let ureq::Error::Io(io) = err {
    return map_io(io);
  }
  Error::Transport(err.to_string())
}

/// A failed read: `TimedOut` becomes `Timeout`, the rest `Transport`.
pub(crate) fn map_io(err: &std::io::Error) -> Error {
  if err.kind() == std::io::ErrorKind::TimedOut {
    Error::Timeout
  } else {
    Error::Transport(err.to_string())
  }
}

#[cfg(test)]
#[path = "tests/error_test.rs"]
mod tests;
