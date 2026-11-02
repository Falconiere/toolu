//! Bounded relay-side RPC over the native command transport.
use super::{
  config,
  store::{Store, Submission, occupied},
};
use serde_json::{Value, json};
use std::path::Path;
use std::time::Duration;
use toolu_runtime::env::Env;
use toolu_state::lock::{LockOptions, with_lock_strict};

mod authority;
mod chunks;
mod verify;
const CHUNK: usize = 256 * 1024;
const LIMIT: usize = 512 * 1024 * 1024;

/// Execute one request against the authoritative relay ledger.
/// # Errors
/// Returns an error for invalid requests or failed durable operations.
pub fn receive(root: &Path, env: &Env, input: &str) -> Result<Value, String> {
  if input.len() > 8 * 1024 * 1024 {
    return Err("request exceeds 8 MiB".into());
  }
  let value: Value = serde_json::from_str(input).map_err(|e| e.to_string())?;
  let op = value
    .get("op")
    .and_then(Value::as_str)
    .ok_or("missing operation")?;
  if matches!(
    op,
    "config" | "enroll" | "retire" | "chunk" | "submit" | "cancel" | "tick"
  ) {
    super::durable::ensure_dir(root)?;
    let options = LockOptions {
      stale: Duration::MAX,
      ..LockOptions::default()
    };
    return with_lock_strict(&root.join("rpc"), options, || {
      dispatch(root, env, op, &value)
    })?;
  }
  dispatch(root, env, op, &value)
}

fn dispatch(root: &Path, env: &Env, op: &str, value: &Value) -> Result<Value, String> {
  if matches!(op, "chunk" | "submit" | "tick") {
    authority::require(root)?;
  }
  let store = Store::new(root);
  match op {
    "authority" => {
      authority::require(root)?;
      Ok(json!({}))
    }
    "configuration" => authority::configuration(root),
    "config" => authority::configure(
      root,
      &store,
      &field(value, "expected")?,
      field(value, "config")?,
    ),
    "enroll" => authority::enroll(
      root,
      &store,
      &field(value, "expected")?,
      field(value, "config")?,
    ),
    "retire" => authority::retire(
      root,
      &store,
      &field(value, "expected")?,
      &field::<String>(value, "successor")?,
    ),
    "list" => encode(store.list()?),
    "status" => super::status::status(root, env),
    "tick" => encode(super::driver::tick(root, env)?),
    "get" => encode(store.load(key(value)?)?),
    "cancel" => cancel(root, &store, key(value)?),
    "chunk" => {
      chunks::put(
        root,
        key(value)?,
        field(value, "index")?,
        &field::<Vec<u8>>(value, "bytes")?,
      )?;
      Ok(json!({}))
    }
    "submit" => submit(root, env, &store, value),
    "output" => output(&store, value),
    _ => Err("unknown operation".into()),
  }
}

fn cancel(root: &Path, store: &Store, key: &str) -> Result<Value, String> {
  let task = store.load(key)?;
  if matches!(task.phase.as_str(), "done" | "failed" | "cancelled") {
    return encode(task);
  }
  authority::require(root)?;
  encode(store.cancel(key)?)
}

/// Verify that the local relay remains an active authority.
/// # Errors
/// The marker is absent, mismatched, or retired.
pub fn check_authority(root: &Path) -> Result<(), String> {
  authority::require(root)
}
fn submit(root: &Path, env: &Env, store: &Store, value: &Value) -> Result<Value, String> {
  let request: Submission = field(value, "request")?;
  let bytes = chunks::assemble(
    root,
    &request.key,
    field(value, "chunks")?,
    field(value, "size")?,
  )?;
  verify::input(root, env, &request, &bytes)?;
  encode(store.submit(request, &bytes)?)
}
fn output(store: &Store, value: &Value) -> Result<Value, String> {
  let offset: u64 = field(value, "offset")?;
  let (size, bytes) = store.output_chunk(key(value)?, offset, CHUNK)?;
  Ok(json!({"bytes": bytes, "size": size}))
}
fn key(value: &Value) -> Result<&str, String> {
  value
    .get("key")
    .and_then(Value::as_str)
    .ok_or_else(|| "missing key".into())
}
fn field<T: serde::de::DeserializeOwned>(value: &Value, name: &str) -> Result<T, String> {
  serde_json::from_value(
    value
      .get(name)
      .cloned()
      .ok_or_else(|| format!("missing {name}"))?,
  )
  .map_err(|e| format!("invalid {name}: {e}"))
}
fn encode<T: serde::Serialize>(value: T) -> Result<Value, String> {
  serde_json::to_value(value).map_err(|e| e.to_string())
}

#[cfg(test)]
#[path = "tests/rpc_test.rs"]
mod tests;
