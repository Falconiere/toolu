//! The raw stdin payload each host gives a hook. Every type is open: all
//! fields are optional, an unknown key is kept in `rest`, and a scalar string
//! field is a [`LenientString`], so any JSON object parses. TypeScript reads the
//! same payloads without a schema (`packages/toolu-core/src/dispatch/dispatch-context.ts`),
//! and the hosts add fields over time.

use serde::{Deserialize, Deserializer};
use serde_json::Value;

use crate::host::Host;

mod cursor;
mod hermes;
mod hook;
mod opencode;

pub use cursor::CursorPayload;
pub use hermes::HermesPayload;
pub use hook::HookPayload;
pub use opencode::{OpencodeInput, OpencodePayload};

/// A string field read leniently: a JSON string reads as itself, and any other
/// value reads as absent, as TypeScript's `typeof value === "string"` reads do.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LenientString(Option<String>);

impl LenientString {
  /// The string, when the field held one (it may be empty).
  pub fn as_str(&self) -> Option<&str> {
    self.0.as_deref()
  }

  /// The string, when the field held a non-empty one: TypeScript's
  /// `text(value, fallback)` without the fallback.
  pub fn text(&self) -> Option<&str> {
    self.as_str().filter(|text| !text.is_empty())
  }
}

impl<'de> Deserialize<'de> for LenientString {
  fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<LenientString, D::Error> {
    Ok(match Value::deserialize(deserializer)? {
      Value::String(text) => LenientString(Some(text)),
      Value::Null | Value::Bool(_) | Value::Number(_) | Value::Array(_) | Value::Object(_) => {
        LenientString(None)
      }
    })
  }
}

/// One host's hook payload.
#[derive(Debug, Clone, PartialEq)]
pub enum Payload {
  /// Claude Code.
  Claude(HookPayload),
  /// Codex.
  Codex(HookPayload),
  /// Cursor.
  Cursor(CursorPayload),
  /// Hermes.
  Hermes(HermesPayload),
  /// `OpenCode`, as the SDK's `(input, output)` pair.
  Opencode(OpencodePayload),
}

/// Parse `text` as `host`'s payload.
///
/// # Errors
/// When `text` is not one JSON object, has a duplicate key for a named field,
/// or holds a lone-surrogate escape.
pub fn parse(host: Host, text: &str) -> Result<Payload, serde_json::Error> {
  Ok(match host {
    Host::Claude => Payload::Claude(serde_json::from_str(text)?),
    Host::Codex => Payload::Codex(serde_json::from_str(text)?),
    Host::Cursor => Payload::Cursor(serde_json::from_str(text)?),
    Host::Hermes => Payload::Hermes(serde_json::from_str(text)?),
    Host::Opencode => Payload::Opencode(serde_json::from_str(text)?),
  })
}

#[cfg(test)]
#[path = "tests/payload_test.rs"]
mod tests;
