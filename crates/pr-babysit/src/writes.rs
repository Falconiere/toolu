//! The agent's GitHub writes, each recorded in the state under the slot lock:
//! one reply per thread, conversation comment or review (a second is refused),
//! and a thread resolve recorded only once GitHub confirms it.

use std::path::Path;
use std::time::Duration;

use serde_json::{Value, json};
use toolu_github::Client;
use toolu_runtime::env::Env;

use crate::collect::failure;
use crate::error::{Failure, Result};
use crate::state::{actions, open, target, write_json};

const RESOLVE: &str = "mutation($threadId:ID!){ resolveReviewThread(input:{threadId:$threadId}){ \
                       thread{ id isResolved } } }";
const RESOLVE_ATTEMPTS: u32 = 3;

mod reply;

pub(crate) use reply::{reply, reply_existing};

/// What a reply answers.
pub(crate) enum Target<'a> {
  /// A review thread: its id, root comment and the comment answered.
  Thread {
    /// The thread's GraphQL id.
    thread: &'a str,
    /// The thread's first comment, by database id.
    root: u64,
    /// The comment answered, by database id.
    in_reply_to: u64,
  },
  /// A conversation comment, by id.
  Conversation(u64),
  /// A review, by id.
  Review(u64),
}

impl Target<'_> {
  fn key(&self) -> String {
    match self {
      Target::Thread {
        thread,
        in_reply_to,
        ..
      } => format!("thread:{thread}@{in_reply_to}"),
      Target::Conversation(id) => format!("conversation:{id}"),
      Target::Review(id) => format!("review:{id}"),
    }
  }

  fn kind(&self) -> &'static str {
    match self {
      Target::Thread { .. } => "thread",
      Target::Conversation(_) => "conversation",
      Target::Review(_) => "review",
    }
  }
}

/// A state file, its environment and the time recorded with a write.
pub(crate) struct Slot<'a> {
  /// The state file.
  pub(crate) path: &'a Path,
  /// The environment the slot lock reads.
  pub(crate) env: &'a Env,
  /// The time recorded, `YYYY-MM-DDTHH:MM:SSZ`.
  pub(crate) now: &'a str,
}

/// Resolve `thread` and record it once GitHub reports it resolved, trying up
/// to three times `pause` apart.
///
/// # Errors
/// `resolve_unconfirmed` when GitHub never reports it resolved, the slot's
/// failures, and GitHub's.
pub(crate) fn resolve(
  client: &Client,
  slot: &Slot<'_>,
  thread: &str,
  pause: Duration,
) -> Result<Value> {
  let (_lock, mut state) = open(slot.path, slot.env)?;
  let prior = actions(&mut state, "resolved")?.get(thread).cloned();
  if let Some(prior) = prior.filter(|prior| prior.get("confirmed") == Some(&Value::Bool(true))) {
    let at = prior.get("at").cloned().unwrap_or(Value::Null);
    return Ok(json!({
      "ok": true, "thread": thread, "confirmed": true, "attempts": 0, "alreadyResolved": true,
      "at": at,
    }));
  }
  let mut last = Value::Null;
  for attempt in 1..=RESOLVE_ATTEMPTS {
    if attempt > 1 {
      std::thread::sleep(pause);
    }
    let reply = client
      .graphql(RESOLVE, &json!({ "threadId": thread }))
      .map_err(|err| failure("resolve", &err))?;
    last = reply
      .data
      .pointer("/resolveReviewThread/thread")
      .cloned()
      .unwrap_or(Value::Null);
    if last.get("isResolved") == Some(&Value::Bool(true)) {
      let head = target(&state).2;
      let record = json!({"confirmed": true, "at": slot.now, "attempts": attempt, "headSha": head});
      actions(&mut state, "resolved")?.insert(thread.to_owned(), record);
      write_json(slot.path, &state, "state")?;
      return Ok(json!({"ok": true, "thread": thread, "confirmed": true, "attempts": attempt}));
    }
  }
  Err(
    Failure::new(
      "resolve_unconfirmed",
      format!("{thread} still unresolved after {RESOLVE_ATTEMPTS} attempt(s)"),
    )
    .with("thread", thread)
    .with("attempts", RESOLVE_ATTEMPTS)
    .with("lastResponse", last),
  )
}

#[cfg(test)]
#[path = "tests/writes_test.rs"]
mod tests;
