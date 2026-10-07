//! `Auth`: the credential attached to a request's first hop only. Its `Debug`
//! never prints a secret.

use std::fmt;

use base64::Engine;

/// Authentication attached to the initial request only.
#[derive(Clone, Default, PartialEq, Eq)]
pub enum Auth {
  /// No Authorization header.
  #[default]
  None,
  /// HTTP basic authentication.
  Basic {
    /// Username, possibly empty.
    username: String,
    /// Password, possibly empty.
    password: String,
  },
  /// HTTP bearer authentication.
  Bearer(String),
}

impl Auth {
  /// The `Authorization` header value, if any.
  pub(crate) fn header_value(&self) -> Option<String> {
    match self {
      Self::None => None,
      Self::Basic { username, password } => {
        let encoded =
          base64::engine::general_purpose::STANDARD.encode(format!("{username}:{password}"));
        Some(format!("Basic {encoded}"))
      }
      Self::Bearer(token) => Some(format!("Bearer {token}")),
    }
  }
}

impl fmt::Debug for Auth {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let redacted = format_args!("<redacted>");
    match self {
      Self::None => f.write_str("None"),
      Self::Basic { username, .. } => f
        .debug_struct("Basic")
        .field("username", username)
        .field("password", &redacted)
        .finish(),
      Self::Bearer(_) => f.debug_tuple("Bearer").field(&redacted).finish(),
    }
  }
}

#[cfg(test)]
#[path = "tests/auth_test.rs"]
mod tests;
