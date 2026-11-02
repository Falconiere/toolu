//! Peer protocol records. Credentials never appear in public session records.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// An opaque session identity and its incarnation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Identity {
  pub id: String,
  pub generation: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
/// A terminal and foreground process labelled by the enrolled session.
pub(super) struct Wake {
  pub address: String,
  pub owner: String,
  pub created_ms: u64,
  #[serde(default)]
  pub pid: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
/// Public membership details, excluding session and group credentials.
pub(super) struct Session {
  pub id: String,
  pub generation: String,
  pub machine: String,
  pub host: String,
  pub group: String,
  pub active: bool,
  pub wake: Option<Wake>,
}

impl Session {
  /// Project the addressable session incarnation.
  pub(super) fn identity(&self) -> Identity {
    Identity {
      id: self.id.clone(),
      generation: self.generation.clone(),
    }
  }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
/// Stored membership and its private session capability.
pub(super) struct Registration {
  pub session: Session,
  pub secret: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
/// Caller proof for an enrolled session incarnation.
pub(super) struct Auth {
  pub id: String,
  pub generation: String,
  pub secret: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
/// Immutable, versioned message content.
pub(super) struct Envelope {
  pub version: u32,
  pub sender: Identity,
  pub recipient: Identity,
  pub group: String,
  pub conversation_id: String,
  pub message_id: String,
  pub reply_to: Option<String>,
  pub timestamp: u64,
  pub text: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
/// Durable inbox entry and independent acknowledgment and wake receipts.
pub(super) struct Message {
  pub key: String,
  pub envelope: Envelope,
  pub acknowledged: bool,
  pub wake: String,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
/// Peer-only state, versioned separately from task and configuration ledgers.
pub(super) struct Ledger {
  pub version: u32,
  #[serde(default)]
  pub wake_cursor: Option<String>,
  pub groups: BTreeMap<String, String>,
  pub sessions: BTreeMap<String, Registration>,
  pub messages: BTreeMap<String, Message>,
}

impl Default for Ledger {
  fn default() -> Self {
    Self {
      version: 1,
      wake_cursor: None,
      groups: BTreeMap::new(),
      sessions: BTreeMap::new(),
      messages: BTreeMap::new(),
    }
  }
}

#[derive(Debug, Serialize)]
/// A bounded protocol failure, with explicit retryability.
pub(super) struct Error {
  pub code: String,
  pub message: String,
  pub retryable: bool,
}

impl Error {
  /// Construct a peer rejection without echoing credentials.
  pub(super) fn new(code: &str, message: &str) -> Self {
    Self {
      code: code.into(),
      message: message.into(),
      retryable: code == "storage",
    }
  }

  /// Record a transient persistence failure.
  pub(super) fn storage(message: String) -> Self {
    Self {
      code: "storage".into(),
      message,
      retryable: true,
    }
  }
}

/// Identifiers usable as map keys and private path components.
pub(super) fn safe(value: &str) -> bool {
  !value.is_empty()
    && value.len() <= 128
    && value
      .bytes()
      .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

/// Bounded printable group/session credentials.
pub(super) fn credential(value: &str) -> bool {
  (32..=256).contains(&value.len()) && value.bytes().all(|byte| byte.is_ascii_graphic())
}

#[cfg(test)]
#[path = "tests/types_test.rs"]
mod tests;
