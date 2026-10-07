//! `Reply`: what the HTTPS server answers for one request.

use std::time::Duration;

/// A response emitted by the HTTPS server for an exact path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply {
  /// HTTP status code.
  pub status: u16,
  /// Response headers.
  pub headers: Vec<(String, String)>,
  /// Response bytes.
  pub body: Vec<u8>,
  /// Delay before writing the response.
  pub delay: Duration,
  /// Close the connection after reading the request, without a response and
  /// without recording the request.
  pub dropped: bool,
}

impl Reply {
  /// A status and body without extra headers or delay.
  pub fn new(status: u16, body: impl Into<Vec<u8>>) -> Self {
    Self {
      status,
      headers: Vec::new(),
      body: body.into(),
      delay: Duration::ZERO,
      dropped: false,
    }
  }

  /// A connection the server closes without answering, as a dropped link.
  pub fn dropped() -> Self {
    Self {
      dropped: true,
      ..Self::new(0, Vec::new())
    }
  }

  /// Add one response header.
  #[must_use]
  pub fn header(mut self, name: &str, value: &str) -> Self {
    self.headers.push((name.into(), value.into()));
    self
  }

  /// Delay the response to exercise a client deadline.
  #[must_use]
  pub fn delayed(mut self, delay: Duration) -> Self {
    self.delay = delay;
    self
  }
}

#[cfg(test)]
#[path = "tests/reply_test.rs"]
mod tests;
