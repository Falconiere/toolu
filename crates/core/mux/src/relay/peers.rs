//! Versioned peer conversations hosted by the existing authenticated relay.

/// Durable sender sessions, outboxes and receipt recovery.
pub mod client;
mod client_delivery;
mod client_model;
mod client_sync;
/// Conservative enrollment of the host’s own Herdr terminal.
pub mod client_wake;
mod membership;
mod messages;
mod storage;
mod types;
/// Safe, at-most-once idle-session notification attempts.
pub mod wake;

use super::{config, rpc};
use serde_json::{Value, json};
use std::path::Path;
use types::{Auth, Error, Ledger};

/// Receive one authenticated transport request with structured peer errors.
pub(super) fn receive(root: &Path, request: &Value) -> Value {
  match execute(root, request) {
    Ok(result) => json!({"version":1,"result":result}),
    Err(error) => json!({"version":1,"error":error}),
  }
}

fn execute(root: &Path, request: &Value) -> Result<Value, Error> {
  validate_request(request)?;
  if request.get("version").and_then(Value::as_u64) != Some(1) {
    return Err(Error::new("invalid", "unsupported peer protocol version"));
  }
  rpc::check_authority(root)
    .map_err(|_error| Error::new("stale", "relay authority is unavailable"))?;
  let cfg = config::load(root).map_err(Error::storage)?;
  if request.get("authority").and_then(Value::as_str) != cfg.generation.as_deref() {
    return Err(Error::new("stale", "relay authority generation differs"));
  }
  let generation = cfg
    .generation
    .as_deref()
    .ok_or_else(|| Error::new("stale", "missing authority generation"))?;
  storage::edit(root, generation, |ledger| dispatch(ledger, &cfg, request))
}

fn validate_request(request: &Value) -> Result<(), Error> {
  let action = request.get("action").and_then(Value::as_str).unwrap_or("");
  let fields: &[&str] = match action {
    "join" => &["session", "secret", "group_key"],
    "list" | "leave" => &["auth"],
    "send" => &["auth", "envelope"],
    "inbox" => &["auth", "after", "unread"],
    "ack" | "status" => &["auth", "key"],
    _ => return Err(Error::new("invalid", "unknown peer operation")),
  };
  let object = request
    .as_object()
    .ok_or_else(|| Error::new("invalid", "request must be an object"))?;
  if object.keys().any(|key| {
    !["op", "version", "authority", "action"].contains(&key.as_str())
      && !fields.contains(&key.as_str())
  }) {
    return Err(Error::new("invalid", "unknown peer request field"));
  }
  Ok(())
}

fn dispatch(ledger: &mut Ledger, cfg: &config::Config, request: &Value) -> Result<Value, Error> {
  let action: String = field(request, "action")?;
  if action == "join" {
    return membership::join(ledger, cfg, request);
  }
  let auth: Auth = field(request, "auth")?;
  let session = membership::authenticate(ledger, cfg, &auth, action == "leave")?;
  match action.as_str() {
    "leave" => membership::leave(ledger, &session),
    "list" => encode(
      ledger
        .sessions
        .values()
        .filter(|entry| {
          entry.session.active
            && entry.session.group == session.group
            && membership::registered(cfg, &entry.session)
        })
        .map(|entry| &entry.session)
        .collect::<Vec<_>>(),
    ),
    "send" => messages::send(ledger, cfg, &session, field(request, "envelope")?),
    "inbox" => messages::inbox(ledger, &session, request),
    "ack" | "status" => messages::receipt(
      ledger,
      &session,
      &field::<String>(request, "key")?,
      action == "ack",
    ),
    _ => Err(Error::new("invalid", "unknown peer operation")),
  }
}

fn field<T: serde::de::DeserializeOwned>(value: &Value, name: &str) -> Result<T, Error> {
  serde_json::from_value(
    value
      .get(name)
      .cloned()
      .ok_or_else(|| Error::new("invalid", &format!("missing {name}")))?,
  )
  .map_err(|_error| Error::new("invalid", &format!("invalid {name}")))
}

fn encode(value: impl serde::Serialize) -> Result<Value, Error> {
  serde_json::to_value(value).map_err(|error| Error::storage(error.to_string()))
}

#[cfg(test)]
#[path = "tests/peers_test.rs"]
mod tests;
