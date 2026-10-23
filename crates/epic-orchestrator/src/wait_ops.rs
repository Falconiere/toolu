//! The engine side of `toolu epic wait` (#588): a wait scoped to one
//! registered epic takes only that epic's attention items and the
//! engine-wide ones, and every reply that hands out an item journals its
//! token, so the rebuild after a restart knows the item was delivered.

use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::model::World;
use crate::server::Engine;
use crate::socket_loop::Waiter;
use crate::status::wait_body;
use crate::wait_journal::replied;

/// Answer a `wait` request now, or park it as a waiter until its deadline.
///
/// # Errors
/// The journal cannot be written; the reply was already sent.
pub(crate) fn reply_wait(
  engine: &mut Engine,
  request: &Value,
  reply: Sender<Value>,
  waiters: &mut Vec<Waiter>,
) -> Result<bool, String> {
  let max_seconds = request
    .get("max_seconds")
    .and_then(Value::as_u64)
    .unwrap_or(0);
  let epic = match scope(&engine.world, request) {
    Ok(epic) => epic,
    Err(err) => {
      let _sent = reply.send(json!({"error": err}));
      return Ok(false);
    }
  };
  let (body, token) = wait_body(&mut engine.world, epic.as_deref());
  if !token.is_empty() || max_seconds == 0 {
    let _sent = reply.send(body.clone());
    drop(reply);
    if !token.is_empty() {
      replied(engine, &body, &token)?;
    }
    return Ok(false);
  }
  let now = Instant::now();
  waiters.push(Waiter {
    deadline: now
      .checked_add(Duration::from_secs(max_seconds))
      .unwrap_or(now),
    reply,
    epic,
  });
  Ok(false)
}

/// The registered epic a request's `epic` selector names (its key, or a
/// trailing issue number); `None` without a selector.
fn scope(world: &World, request: &Value) -> Result<Option<String>, String> {
  let Some(selector) = request.get("epic").and_then(Value::as_str) else {
    return Ok(None);
  };
  world
    .epics
    .keys()
    .find(|key| *key == selector || key.ends_with(&format!("-{selector}")))
    .cloned()
    .map(Some)
    .ok_or_else(|| format!("unknown epic: {selector}"))
}

/// Reply to every waiter whose scope has an item, and time out the rest
/// past their deadline, journaling each reply as a wake-up.
///
/// # Errors
/// The journal cannot be written; the replies were already sent.
pub(crate) fn satisfy(engine: &mut Engine, waiters: &mut Vec<Waiter>) -> Result<(), String> {
  let now = Instant::now();
  let mut staying = Vec::new();
  let mut sent = Vec::new();
  for waiter in waiters.drain(..) {
    let (body, token) = wait_body(&mut engine.world, waiter.epic.as_deref());
    if token.is_empty() && waiter.deadline > now {
      staying.push(waiter);
      continue;
    }
    let _sent = waiter.reply.send(body.clone());
    sent.push((body, token));
  }
  *waiters = staying;
  sent
    .iter()
    .try_for_each(|(body, token)| replied(engine, body, token))
}

/// Reply to waiters whose deadlines have passed.
///
/// # Errors
/// As [`satisfy`].
pub(crate) fn expire(engine: &mut Engine, waiters: &mut Vec<Waiter>) -> Result<(), String> {
  satisfy(engine, waiters)
}

#[cfg(test)]
#[path = "tests/wait_ops_test.rs"]
mod tests;
