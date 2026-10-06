//! A JSON object written key by key, so the keys keep TypeScript's order;
//! `serde_json`'s `Value` display escapes every string.

use serde_json::Value;

/// A JSON object written key by key, in order, as `JSON.stringify` writes it.
pub(super) struct Object(String);

impl Object {
  pub(super) fn new() -> Object {
    Object(String::from("{"))
  }

  fn key(&mut self, key: &str) {
    if self.0.len() > 1 {
      self.0.push(',');
    }
    self.0.push_str(&Value::from(key).to_string());
    self.0.push(':');
  }

  pub(super) fn text(mut self, key: &str, value: &str) -> Object {
    self.key(key);
    self.0.push_str(&Value::from(value).to_string());
    self
  }

  pub(super) fn flag(mut self, key: &str, value: bool) -> Object {
    self.key(key);
    self.0.push_str(if value { "true" } else { "false" });
    self
  }

  pub(super) fn object(mut self, key: &str, value: Object) -> Object {
    self.key(key);
    self.0.push_str(&value.close());
    self
  }

  pub(super) fn close(mut self) -> String {
    self.0.push('}');
    self.0
  }

  pub(super) fn line(self) -> String {
    let mut line = self.close();
    line.push('\n');
    line
  }
}

#[cfg(test)]
#[path = "tests/object_test.rs"]
mod tests;
