//! `OpenCode` output: the before hook can continue or throw, and neither grants a
//! native permission.

use super::{Callback, CallbackAction, Normal, Object};

/// The callback for `normal`: a refusal throws, everything else continues.
pub(super) fn callback(normal: Normal<'_>) -> Callback {
  let action = match normal {
    Normal::Deny(_) | Normal::Ask(_) => CallbackAction::Throw,
    Normal::Allow | Normal::Advisory(_) | Normal::Block(_) => CallbackAction::Continue,
  };
  let message = Some(normal.text())
    .filter(|text| !text.is_empty())
    .map(str::to_owned);
  Callback { action, message }
}

impl Callback {
  /// The callback as TypeScript's `EncodedOutput` object: `{"kind":"callback",…}`.
  pub fn json(&self) -> String {
    let action = match self.action {
      CallbackAction::Continue => "continue",
      CallbackAction::Throw => "throw",
    };
    let object = Object::new()
      .text("kind", "callback")
      .text("action", action);
    match &self.message {
      Some(message) => object.text("message", message).close(),
      None => object.close(),
    }
  }
}

#[cfg(test)]
#[path = "tests/opencode_test.rs"]
mod tests;
