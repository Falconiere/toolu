//! The structured error every `toolu babysit` verb reports: one closed `code`,
//! a message and extra fields, printed as `{"version":1,"errors":[…]}` on
//! stdout with the exit its code maps to.

use serde_json::{Map, Value, json};
use toolu_protocol::exit::Exit;
use toolu_runtime::cli::Outcome;

/// A failed verb: the code, the message and the extra fields of its document.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Failure {
  /// The closed error code, such as `locked` or `api_error`.
  pub(crate) code: &'static str,
  /// The message fit to show a user.
  pub(crate) message: String,
  /// Extra document fields, such as `pid` or `source`.
  pub(crate) extra: Map<String, Value>,
}

/// A verb's result.
pub(crate) type Result<T> = std::result::Result<T, Failure>;

impl Failure {
  /// A failure with no extra fields.
  pub(crate) fn new(code: &'static str, message: impl Into<String>) -> Self {
    Failure {
      code,
      message: message.into(),
      extra: Map::new(),
    }
  }

  /// A `usage` failure.
  pub(crate) fn usage(message: impl Into<String>) -> Self {
    Failure::new("usage", message)
  }

  /// This failure with one more extra field.
  pub(crate) fn with(mut self, key: &str, value: impl Into<Value>) -> Self {
    self.extra.insert(key.to_owned(), value.into());
    self
  }

  /// The exit this code maps to.
  pub(crate) fn exit(&self) -> Exit {
    match self.code {
      "usage" => Exit::Usage,
      "gh_unavailable" | "herdr_unavailable" => Exit::Unavailable,
      "locked" | "api_error" | "head_moved" | "resolve_unconfirmed" | "reply_uncertain" => {
        Exit::TempFail
      }
      _ => Exit::Failure,
    }
  }

  /// `{"version":1,"errors":[{code,message,…extra}]}`.
  pub(crate) fn document(&self) -> Value {
    let mut entry = Map::new();
    entry.insert("code".into(), Value::from(self.code));
    entry.insert("message".into(), Value::from(self.message.clone()));
    for (key, value) in &self.extra {
      entry.insert(key.clone(), value.clone());
    }
    json!({"version": 1, "errors": [Value::Object(entry)]})
  }

  /// The document on stdout and the message on stderr, with the mapped exit.
  pub(crate) fn outcome(&self) -> Outcome {
    Outcome {
      exit: self.exit(),
      stdout: Some(self.document().to_string()),
      stderr: Some(format!("babysit: {}", self.message)),
    }
  }
}

impl std::fmt::Display for Failure {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}: {}", self.code, self.message)
  }
}

impl std::error::Error for Failure {}

/// A verb's JSON value as a successful outcome, or its failure.
pub(crate) fn finish(result: Result<Value>) -> Outcome {
  match result {
    Ok(value) => Outcome::data(value.to_string()),
    Err(failure) => failure.outcome(),
  }
}

#[cfg(test)]
#[path = "tests/error_test.rs"]
mod tests;
