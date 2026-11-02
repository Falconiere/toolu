use super::types::{Envelope, Message, Session, safe};
use crate::relay::write_state;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;
use std::{collections::BTreeMap, path::Path};

#[derive(Serialize, Deserialize)]
/// Persisted local registration and its pinned relay.
pub(super) struct State {
  pub version: u32,
  pub session: Session,
  pub secret: String,
  pub relay: String,
  pub authority: String,
  pub group_key: String,
}
#[derive(Serialize, Deserialize)]
/// Versioned local outgoing messages.
pub(super) struct Outbox {
  pub version: u32,
  pub messages: BTreeMap<String, Outgoing>,
}
#[derive(Clone, Serialize, Deserialize)]
/// An immutable envelope and its delivery receipt.
pub(super) struct Outgoing {
  pub envelope: Envelope,
  pub delivery: String,
  pub receipt: Receipt,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub message: Option<Message>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub error: Option<Value>,
}
#[derive(Clone, Serialize, Deserialize)]
/// Durable sender, inbox and acknowledgment receipt.
pub(super) struct Receipt {
  pub outbox: bool,
  pub inbox: bool,
  pub acknowledged: bool,
}
#[derive(Deserialize)]
/// Versioned relay response.
pub(super) struct Response<T> {
  pub version: u32,
  pub result: Option<T>,
  pub error: Option<RemoteError>,
}
#[derive(Serialize, Deserialize)]
/// Structured relay rejection or retryable error.
pub(super) struct RemoteError {
  pub code: String,
  pub message: String,
  pub retryable: bool,
}
/// Read one serialized local record.
pub(super) fn load<T: DeserializeOwned>(path: &Path) -> Result<T, String> {
  serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?)
    .map_err(|e| e.to_string())
}
/// Atomically persist one local record.
pub(super) fn save(path: &Path, value: &impl Serialize) -> Result<(), String> {
  write_state(path, &serde_json::to_vec(value).map_err(|e| e.to_string())?)
}
/// Read and validate a saved session.
pub(super) fn state(path: &Path) -> Result<State, String> {
  let state: State = load(path)?;
  let session = &state.session;
  if state.version != 1
    || ![
      &session.id,
      &session.generation,
      &session.group,
      &session.machine,
    ]
    .into_iter()
    .all(|s| safe(s))
    || path
      .parent()
      .and_then(Path::file_name)
      .and_then(|name| name.to_str())
      != Some(session.id.as_str())
    || !session.active
  {
    return Err("invalid peer session state".into());
  }
  Ok(state)
}
/// Read and validate a saved outbox.
pub(super) fn outbox(path: &Path) -> Result<Outbox, String> {
  let outbox: Outbox = load(path)?;
  if outbox.version != 1 {
    return Err("unsupported local peer outbox version".into());
  }
  Ok(outbox)
}
/// Convert a local record to the public JSON representation.
pub(super) fn encode(value: &impl Serialize) -> Result<Value, String> {
  serde_json::to_value(value).map_err(|e| e.to_string())
}

#[cfg(test)]
#[path = "tests/client_model_test.rs"]
mod tests;
