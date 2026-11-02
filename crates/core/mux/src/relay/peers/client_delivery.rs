use super::types::Message;
use super::{
  client::{auth, rpc},
  client_model::{Outbox, Outgoing, Response, State, encode, save},
};
use serde_json::{Value, json};
use std::{path::Path, time::Duration};
use toolu_runtime::env::Env;

/// Attempt one persisted message and save its receipt.
pub(super) fn deliver(
  env: &Env,
  state: &State,
  saved: (&Path, &mut Outbox),
  id: &str,
  timeout: Duration,
) -> Result<Value, String> {
  let (path, outbox) = saved;
  let item = outbox
    .messages
    .get_mut(id)
    .ok_or("missing outbox message")?;
  if item.delivery != "queued" {
    return encode(item);
  }
  let response = rpc(
    env,
    state,
    json!({"action":"send","auth":auth(state),"envelope":item.envelope}),
    timeout,
  );
  match response {
    Err(error) => item.error = Some(json!(error)),
    Ok(value) => apply_response(item, &state.session.id, id, value)?,
  }
  let result = encode(item)?;
  save(path, outbox)?;
  Ok(result)
}
fn apply_response(item: &mut Outgoing, sender: &str, id: &str, value: Value) -> Result<(), String> {
  let Ok(response) = serde_json::from_value::<Response<Message>>(value) else {
    item.error = Some(json!("invalid peer response"));
    return Ok(());
  };
  if response.version != 1 {
    item.error = Some(json!("invalid peer response"));
    return Ok(());
  }
  if let Some(error) = response.error {
    if !error.retryable {
      item.delivery = "rejected".into();
    }
    item.error = Some(encode(&error)?);
  } else if let Some(message) = response
    .result
    .filter(|m| m.key == format!("{sender}/{id}") && m.envelope == item.envelope)
  {
    record_receipt(item, message);
  } else {
    item.error = Some(json!("invalid peer response"));
  }
  Ok(())
}
/// Replace cached message details and all independent delivery receipts together.
pub(super) fn record_receipt(item: &mut Outgoing, message: Message) {
  item.delivery = if message.acknowledged {
    "acknowledged"
  } else {
    "delivered"
  }
  .into();
  item.error = None;
  item.receipt.inbox = true;
  item.receipt.acknowledged = message.acknowledged;
  item.message = Some(message);
}
#[cfg(test)]
#[path = "tests/client_delivery_test.rs"]
mod tests;
