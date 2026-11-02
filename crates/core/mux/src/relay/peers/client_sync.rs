use super::types::Message;
use super::{
  client::{auth, directory, rpc},
  client_delivery::{deliver, record_receipt},
  client_model::{Outbox, Outgoing, Response, State, encode, outbox, save, state},
};
use serde_json::{Value, json};
use std::{
  collections::BTreeMap,
  time::{Duration, Instant},
};
use toolu_runtime::env::Env;
use toolu_state::lock::{LockOptions, with_lock_strict};

/// Retry queued messages and refresh delivery receipts.
/// # Errors
/// State or local persistence failure.
pub fn sync(
  env: &Env,
  id: &str,
  key: Option<&str>,
  limit: usize,
  timeout: Duration,
) -> Result<Value, String> {
  let deadline = Instant::now()
    .checked_add(timeout)
    .ok_or("invalid timeout")?;
  let dir = directory(env, id)?;
  let state = state(&dir.join("state.json"))?;
  let path = dir.join("outbox.json");
  let options = LockOptions {
    timeout: timeout.min(Duration::from_millis(100)),
    stale: Duration::MAX,
  };
  with_lock_strict(&path, options, || {
    let mut outbox = outbox(&path)?;
    let matches =
      |message_id: &str| key.is_none_or(|k| k == message_id || k == format!("{id}/{message_id}"));
    let queued = outbox
      .messages
      .iter()
      .filter(|(id, item)| matches(id) && item.delivery == "queued");
    let rest = outbox
      .messages
      .iter()
      .filter(|(id, item)| matches(id) && item.delivery != "queued");
    let ids: Vec<String> = queued
      .chain(rest)
      .take(limit)
      .map(|(id, _)| id.clone())
      .collect();
    retry(env, &state, (&path, &mut outbox), &ids, deadline)?;
    save(&path, &outbox)?;
    let messages: BTreeMap<_, _> = ids
      .into_iter()
      .filter_map(|id| outbox.messages.get(&id).cloned().map(|item| (id, item)))
      .collect();
    Ok(json!({"version":1,"messages":messages}))
  })?
}
fn retry(
  env: &Env,
  state: &State,
  saved: (&std::path::Path, &mut Outbox),
  ids: &[String],
  deadline: Instant,
) -> Result<(), String> {
  let (path, outbox) = saved;
  for message_id in ids {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
      break;
    }
    let delivery = outbox
      .messages
      .get(message_id)
      .ok_or("missing outbox message")?
      .delivery
      .clone();
    if delivery == "queued" {
      deliver(env, state, (path, &mut *outbox), message_id, remaining)?;
      if outbox
        .messages
        .get(message_id)
        .is_some_and(|m| m.delivery == "queued")
      {
        break;
      }
    } else if delivery == "delivered" && !refresh(env, state, outbox, message_id, remaining)? {
      break;
    }
  }
  Ok(())
}
fn refresh(
  env: &Env,
  state: &State,
  outbox: &mut Outbox,
  id: &str,
  timeout: Duration,
) -> Result<bool, String> {
  let key = format!("{}/{id}", state.session.id);
  let Ok(value) = rpc(
    env,
    state,
    json!({"action":"status","auth":auth(state),"key":key}),
    timeout,
  ) else {
    return Ok(false);
  };
  let Ok(response) = serde_json::from_value::<Response<Message>>(value) else {
    return Ok(false);
  };
  let item = outbox
    .messages
    .get_mut(id)
    .ok_or("missing outbox message")?;
  if response.version == 1
    && let Some(message) = response.result
    && message.key == key
    && message.envelope == item.envelope
  {
    record_receipt(item, message);
  }
  Ok(true)
}
fn selected(
  outbox: &Outbox,
  id: &str,
  key: Option<&str>,
  limit: usize,
) -> BTreeMap<String, Outgoing> {
  outbox
    .messages
    .iter()
    .filter(|(message_id, _)| {
      key.is_none_or(|k| k == message_id.as_str() || k == format!("{id}/{message_id}"))
    })
    .take(limit)
    .map(|(id, item)| (id.clone(), item.clone()))
    .collect()
}
/// Read saved receipts without contacting the relay.
/// # Errors
/// Local state is unavailable or invalid.
pub fn local(env: &Env, id: &str, key: Option<&str>, limit: usize) -> Result<Value, String> {
  let dir = directory(env, id)?;
  state(&dir.join("state.json"))?;
  let outbox = outbox(&dir.join("outbox.json"))?;
  encode(&json!({"version":1,"messages":selected(&outbox,id,key,limit)}))
}
#[cfg(test)]
#[path = "tests/client_sync_test.rs"]
mod tests;
