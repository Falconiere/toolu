//! `Text`: a string that is never empty, the Rust form of zod's `z.string().min(1)`.

use std::fmt;

use serde::{Deserialize, Serialize};

/// A non-empty string. It reads and writes as a JSON string, and an empty one
/// fails to parse.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Text(String);

/// The error for an empty string where [`Text`] is required.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmptyText;

impl Text {
  /// `text`, when it is not empty.
  ///
  /// # Errors
  /// [`EmptyText`] when `text` is empty.
  pub fn new(text: impl Into<String>) -> Result<Text, EmptyText> {
    let text = text.into();
    if text.is_empty() {
      Err(EmptyText)
    } else {
      Ok(Text(text))
    }
  }

  /// The string.
  pub fn as_str(&self) -> &str {
    &self.0
  }
}

impl TryFrom<String> for Text {
  type Error = EmptyText;

  fn try_from(text: String) -> Result<Text, EmptyText> {
    Text::new(text)
  }
}

impl From<Text> for String {
  fn from(text: Text) -> String {
    text.0
  }
}

impl fmt::Display for EmptyText {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str("must not be empty")
  }
}

impl std::error::Error for EmptyText {}

#[cfg(test)]
#[path = "tests/text_test.rs"]
mod tests;
