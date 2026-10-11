//! The agent's GitHub writes, each recorded in the state under the slot lock:
//! one reply per thread, conversation comment or review (a second is refused),
//! and a thread resolve recorded only once GitHub confirms it.

use std::path::Path;
use std::time::Duration;

use serde_json::{Value, json};
use toolu_github::{Client, Method, Rest, RestRequest};
use toolu_runtime::env::Env;

use crate::collect::failure;
use crate::error::{Failure, Result};
use crate::state::{actions, open, target, write_json};

const RESOLVE: &str = "mutation($threadId:ID!){ resolveReviewThread(input:{threadId:$threadId}){ \
                       thread{ id isResolved } } }";
const RESOLVE_ATTEMPTS: u32 = 3;

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

fn reply_path(repo: &str, number: u64, to: &Target<'_>) -> String {
  match to {
    Target::Thread { root, .. } => format!("/repos/{repo}/pulls/{number}/comments/{root}/replies"),
    Target::Conversation(_) | Target::Review(_) => {
      format!("/repos/{repo}/issues/{number}/comments")
    }
  }
}

/// Post `body` as the one reply to `to`, and record it.
///
/// # Errors
/// `duplicate_reply` when a reply is already recorded, the slot's failures,
/// and GitHub's.
pub(crate) fn reply(
  client: &Client,
  slot: &Slot<'_>,
  to: &Target<'_>,
  body: &str,
) -> Result<Value> {
  let (_lock, mut state) = open(slot.path, slot.env)?;
  let key = to.key();
  let replied = actions(&mut state, "replied")?;
  if let Some(recorded) = replied.get(&key).filter(|recorded| !recorded.is_null()) {
    return Err(
      Failure::new(
        "duplicate_reply",
        format!("a reply to {key} is already recorded; not posting again"),
      )
      .with("key", key.clone())
      .with("recorded", recorded.clone()),
    );
  }
  let (repo, number, head) = target(&state);
  let path = reply_path(repo, number, to);
  let payload = json!({ "body": body });
  let request = RestRequest {
    method: Method::Post,
    path: &path,
    body: Some(&payload),
    etag: None,
  };
  let reply = client
    .rest(&request)
    .map_err(|err| failure("reply", &err))?;
  let Rest::Fresh(fresh) = reply.data else {
    return Err(Failure::new("invalid_json", "reply answered 304").with("source", "reply"));
  };
  let posted: Value = fresh.json().map_err(|err| failure("reply", &err))?;
  let Some(id) = posted.get("id").filter(|id| id.is_u64()).cloned() else {
    return Err(
      Failure::new("invalid_json", "reply response carried no comment id").with("source", "reply"),
    );
  };
  let url = posted.get("html_url").cloned().unwrap_or(Value::Null);
  let record =
    json!({"commentId": id, "url": url, "at": slot.now, "headSha": head, "kind": to.kind()});
  actions(&mut state, "replied")?.insert(key.clone(), record);
  write_json(slot.path, &state, "state")?;
  Ok(json!({"ok": true, "key": key, "commentId": id, "url": url}))
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
