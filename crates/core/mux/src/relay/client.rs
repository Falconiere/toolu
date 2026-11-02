//! Caller-side durable submission and bounded relay transport.
use super::{
  config,
  store::{Store, Submission, Task},
};
use serde_json::{Value, json};
use std::time::Duration;
use toolu_runtime::{
  env::Env,
  process::{Spec, run},
};
mod endpoint;

const CHUNK: usize = 256 * 1024;
const LIMIT: usize = 512 * 1024 * 1024;

/// Send one JSON request to the configured relay over hc.
///
/// # Errors
/// Configuration, transport, or remote request failure.
pub fn call(env: &Env, request: &Value) -> Result<Value, String> {
  let config = config::load(&config::root(env))?;
  let relay = env
    .get("TOOLU_RELAY_TARGET")
    .map(str::to_owned)
    .or(config.relay)
    .ok_or("relay is not configured")?;
  let input = serde_json::to_vec(request).map_err(|e| e.to_string())?;
  if input.len() > 8 * 1024 * 1024 {
    return Err("request exceeds 8 MiB".into());
  }
  let mut spec = Spec::new([
    "hc", "exec", "--on", &relay, "-i", "--", "toolu", "--json", "relay", "receive",
  ]);
  spec.env = Some(env.clone());
  spec.stdin = input;
  spec.timeout = Duration::from_secs(130);
  spec.max_output_bytes = 8 * 1024 * 1024 + 1;
  let out = run(&spec).map_err(|e| format!("{e:?}"))?;
  if out.timed_out || out.truncated || out.stdout_bytes.len() > 8 * 1024 * 1024 {
    return Err("relay transport timed out or exceeded 8 MiB".into());
  }
  let value: Value = serde_json::from_slice(&out.stdout_bytes)
    .map_err(|e| format!("invalid relay response: {e}"))?;
  if out.exit_code != 0 {
    return Err(
      value
        .pointer("/error/message")
        .and_then(Value::as_str)
        .unwrap_or("relay command failed")
        .to_owned(),
    );
  }
  Ok(value)
}

/// Persist the submission before sending immutable chunks and accepting its receipt.
///
/// # Errors
/// Invalid input, conflicting local receipt, or transport failure.
pub fn submit(env: &Env, request: &Submission, bundle: &[u8]) -> Result<Task, String> {
  Store::new(&config::root(env).join("outbox")).submit(request.clone(), bundle)?;
  let env = env
    .clone()
    .with("TOOLU_RELAY_TARGET", &endpoint::pin(env, &request.key)?);
  for (index, bytes) in bundle.chunks(CHUNK).enumerate() {
    call(
      &env,
      &json!({"op":"chunk", "key":request.key, "index":index, "bytes":bytes}),
    )?;
  }
  let value = call(
    &env,
    &json!({"op":"submit", "request":request, "chunks":bundle.len().div_ceil(CHUNK), "size":bundle.len()}),
  )?;
  serde_json::from_value(value).map_err(|e| e.to_string())
}

/// Fetch a task receipt.
/// # Errors
/// The task or relay cannot be read.
pub fn get(env: &Env, key: &str) -> Result<Task, String> {
  let env = env
    .clone()
    .with("TOOLU_RELAY_TARGET", &endpoint::get(env, key)?);
  serde_json::from_value(call(&env, &json!({"op":"get", "key":key}))?).map_err(|e| e.to_string())
}

/// Request task cancellation.
/// # Errors
/// The cancellation or relay transport fails.
pub fn cancel(env: &Env, key: &str) -> Result<Task, String> {
  let env = env
    .clone()
    .with("TOOLU_RELAY_TARGET", &endpoint::get(env, key)?);
  serde_json::from_value(call(&env, &json!({"op":"cancel", "key":key}))?).map_err(|e| e.to_string())
}

/// Fetch the complete verified result in bounded chunks.
/// # Errors
/// Invalid or inconsistent relay output, or transport failure.
pub fn output(env: &Env, key: &str) -> Result<Vec<u8>, String> {
  let env = env
    .clone()
    .with("TOOLU_RELAY_TARGET", &endpoint::get(env, key)?);
  let mut result = Vec::new();
  let mut expected = None;
  loop {
    let value = call(
      &env,
      &json!({"op":"output", "key":key, "offset":result.len()}),
    )?;
    let size: usize = serde_json::from_value(value.get("size").cloned().ok_or("missing size")?)
      .map_err(|e| format!("invalid size: {e}"))?;
    let bytes: Vec<u8> =
      serde_json::from_value(value.get("bytes").cloned().ok_or("missing bytes")?)
        .map_err(|e| format!("invalid bytes: {e}"))?;
    if size > LIMIT
      || expected.is_some_and(|previous| previous != size)
      || result.len() > size
      || bytes.len() != CHUNK.min(size - result.len())
    {
      return Err("invalid relay output chunk".into());
    }
    expected = Some(size);
    result.extend_from_slice(&bytes);
    if result.len() == size {
      return Ok(result);
    }
  }
}

/// Publish the local worker registry to the authoritative relay.
/// # Errors
/// The relay rejects the configuration or transport fails.
pub fn publish(
  env: &Env,
  expected: &config::Config,
  config: &config::Config,
) -> Result<(), String> {
  call(
    env,
    &json!({"op":"config", "expected":expected, "config":config}),
  )
  .map(|_| ())
}

/// Enroll an unused or retired relay with a fresh generation.
/// # Errors
/// The target rejects the generation or transport fails.
pub fn enroll(env: &Env, expected: &config::Config, config: &config::Config) -> Result<(), String> {
  call(
    env,
    &json!({"op":"enroll", "expected":expected, "config":config}),
  )
  .map(|_| ())
}

/// Read the target relay's configuration and retirement record.
/// # Errors
/// The target cannot be reached or its metadata is invalid.
pub fn configuration(env: &Env) -> Result<Value, String> {
  call(env, &json!({"op":"configuration"}))
}

#[cfg(test)]
#[path = "tests/client_test.rs"]
mod tests;
