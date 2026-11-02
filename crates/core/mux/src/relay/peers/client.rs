//! Durable caller-side peer sessions and messages.
use super::client_model::{Outbox, Outgoing, Receipt, State, outbox, save, state};
use super::types::{Envelope, Identity, Session, safe};
use crate::relay::{client, config};
use serde_json::{Value, json};
use std::{
  collections::BTreeMap,
  io::Read as _,
  path::{Path, PathBuf},
  time::{Duration, SystemTime, UNIX_EPOCH},
};
use toolu_runtime::env::Env;
use toolu_state::lock::{LockOptions, token, with_lock_strict};

/// Resolve a safe local session directory.
pub(super) fn directory(env: &Env, id: &str) -> Result<PathBuf, String> {
  if !safe(id) {
    return Err("invalid session id".into());
  }
  Ok(config::root(env).join("peers/clients").join(id))
}
fn secret() -> Result<String, String> {
  let mut bytes = [0_u8; 32];
  std::fs::File::open("/dev/urandom")
    .and_then(|mut f| f.read_exact(&mut bytes))
    .map_err(|e| e.to_string())?;
  let mut secret = String::with_capacity(64);
  for byte in bytes {
    use std::fmt::Write as _;
    write!(&mut secret, "{byte:02x}").map_err(|e| e.to_string())?;
  }
  Ok(secret)
}
/// Build the session capability for relay requests.
pub(super) fn auth(state: &State) -> Value {
  json!({"id":state.session.id,"generation":state.session.generation,"secret":state.secret})
}
/// Send a request to the session's pinned relay.
pub(super) fn rpc(
  env: &Env,
  state: &State,
  request: Value,
  timeout: Duration,
) -> Result<Value, String> {
  let Value::Object(mut request) = request else {
    return Err("invalid request".into());
  };
  request.insert("op".into(), json!("peers"));
  request.insert("version".into(), json!(1));
  request.insert("authority".into(), json!(state.authority));
  client::call_timeout(
    &env.clone().with("TOOLU_RELAY_TARGET", &state.relay),
    &Value::Object(request),
    timeout,
  )
}
fn result(response: Value) -> Result<Value, String> {
  let Value::Object(mut response) = response else {
    return Err("invalid peer response".into());
  };
  if response.get("version").and_then(Value::as_u64) != Some(1) {
    return Err("invalid peer response version".into());
  }
  if let Some(error) = response.remove("error") {
    return Err(error.to_string());
  }
  response
    .remove("result")
    .ok_or_else(|| "invalid peer response".into())
}
/// Inputs for creating a peer session.
pub struct Join<'a> {
  /// Conversation group.
  pub group: &'a str,
  /// Registered machine name.
  pub machine: &'a str,
  /// Supported host name.
  pub host: &'a str,
  /// Path to shared group key.
  pub key_file: &'a str,
}
/// Create and persist a session before registration.
/// # Errors
/// Invalid configuration, storage, or transport failure.
pub fn join(env: &Env, input: &Join<'_>, timeout: Duration) -> Result<Value, String> {
  let id = prepare(env, input)?;
  register(env, &id, timeout)
    .map_err(|error| format!("session {id} saved; registration failed: {error}"))
}
/// Persist a new session before its first registration attempt.
/// # Errors
/// Invalid configuration, key, or local storage failure.
pub fn prepare(env: &Env, input: &Join<'_>) -> Result<String, String> {
  if !safe(input.group)
    || !safe(input.machine)
    || !matches!(
      input.host,
      "claude" | "codex" | "muse" | "cursor" | "opencode"
    )
  {
    return Err("invalid peer group, machine or agent".into());
  }
  let config = config::load(&config::root(env))?;
  let relay = env
    .get("TOOLU_RELAY_TARGET")
    .map(str::to_owned)
    .or(config.relay)
    .ok_or("relay is not configured")?;
  let authority = config
    .generation
    .ok_or("relay authority is not configured")?;
  let key = group_key(input.key_file)?;
  let id = token();
  let session = Session {
    id: id.clone(),
    generation: token(),
    machine: input.machine.into(),
    host: input.host.into(),
    group: input.group.into(),
    active: true,
    wake: None,
  };
  let state = State {
    version: 1,
    session,
    secret: secret()?,
    relay,
    authority,
    group_key: key,
  };
  let dir = directory(env, &id)?;
  save(&dir.join("state.json"), &state)?;
  save(
    &dir.join("outbox.json"),
    &Outbox {
      version: 1,
      messages: BTreeMap::new(),
    },
  )?;
  Ok(id)
}
fn group_key(path: &str) -> Result<String, String> {
  let key = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
  let key = key.trim();
  if !(32..=256).contains(&key.len()) || !key.bytes().all(|b| b.is_ascii_graphic()) {
    return Err("group key must contain 32 to 256 printable ASCII bytes".into());
  }
  Ok(key.to_owned())
}
/// Retry a saved registration, including after a lost receipt.
/// # Errors
/// State or registration failure.
pub fn register(env: &Env, id: &str, timeout: Duration) -> Result<Value, String> {
  let state = state(&directory(env, id)?.join("state.json"))?;
  let response = rpc(
    env,
    &state,
    json!({"action":"join","session":state.session,"secret":state.secret,"group_key":state.group_key}),
    timeout,
  )?;
  Ok(json!({"session_id":id,"session":result(response)?}))
}
/// Send an authenticated peer request using its pinned authority.
/// # Errors
/// State, transport, or relay failure.
pub fn request(
  env: &Env,
  id: &str,
  action: &str,
  fields: Value,
  timeout: Duration,
) -> Result<Value, String> {
  let state = state(&directory(env, id)?.join("state.json"))?;
  let Value::Object(mut request) = fields else {
    return Err("invalid request".into());
  };
  request.insert("action".into(), json!(action));
  request.insert("auth".into(), auth(&state));
  result(rpc(env, &state, Value::Object(request), timeout)?)
}
/// Inputs for an outgoing message.
pub struct Send<'a> {
  /// Recipient identity.
  pub recipient: Value,
  /// UTF-8 message text.
  pub text: &'a str,
  /// Optional caller-controlled idempotency key.
  pub message_id: Option<&'a str>,
  /// Original message key for replies.
  pub reply_to: Option<&'a str>,
  /// Conversation id for replies.
  pub conversation_id: Option<&'a str>,
}
/// Persist and attempt delivery of an immutable message.
/// # Errors
/// Invalid input, conflicting ID or local storage failure.
pub fn send(env: &Env, id: &str, input: &Send<'_>) -> Result<Value, String> {
  if input.text.is_empty() || input.text.len() > 65536 {
    return Err("message must be nonempty and at most 64 KiB".into());
  }
  let dir = directory(env, id)?;
  let state = state(&dir.join("state.json"))?;
  let msg_id = input.message_id.map_or_else(token, str::to_owned);
  let recipient: Identity = serde_json::from_value(input.recipient.clone())
    .map_err(|e| format!("invalid recipient: {e}"))?;
  if !safe(&msg_id) || !safe(&recipient.id) || !safe(&recipient.generation) {
    return Err("invalid message or recipient identity".into());
  }
  let timestamp = u64::try_from(
    SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .map_err(|e| e.to_string())?
      .as_millis(),
  )
  .map_err(|e| e.to_string())?;
  let envelope = Envelope {
    version: 1,
    sender: state.session.identity(),
    recipient,
    group: state.session.group.clone(),
    conversation_id: input.conversation_id.unwrap_or(&msg_id).into(),
    message_id: msg_id.clone(),
    reply_to: input.reply_to.map(str::to_owned),
    timestamp,
    text: input.text.into(),
  };
  save_send(env, &state, &dir.join("outbox.json"), &msg_id, envelope)
}
fn save_send(
  env: &Env,
  state: &State,
  path: &Path,
  msg_id: &str,
  mut envelope: Envelope,
) -> Result<Value, String> {
  let options = LockOptions {
    stale: Duration::MAX,
    ..LockOptions::default()
  };
  with_lock_strict(path, options, || {
    let mut outbox = outbox(path)?;
    if let Some(old) = outbox.messages.get(msg_id) {
      envelope.timestamp = old.envelope.timestamp;
      if envelope != old.envelope {
        return Err("message ID conflicts with saved message".into());
      }
    } else {
      outbox.messages.insert(
        msg_id.into(),
        Outgoing {
          envelope,
          delivery: "queued".into(),
          receipt: Receipt {
            outbox: true,
            inbox: false,
            acknowledged: false,
          },
          message: None,
          error: None,
        },
      );
      save(path, &outbox)?;
    }
    deliver(
      env,
      state,
      (path, &mut outbox),
      msg_id,
      Duration::from_secs(130),
    )
  })?
}
use super::client_delivery::deliver;
pub use super::client_sync::{local, sync};
/// Conservative enrollment of the host’s own Herdr terminal.
pub use super::client_wake as wake;
#[cfg(test)]
#[path = "tests/client_test.rs"]
mod tests;
