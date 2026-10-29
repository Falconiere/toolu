//! Durable comment creation: an intent is written before the POST, and an
//! uncertain send is reconciled against remote comments before any later send.

use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};
use toolu_github::{Client, Error, Method, Rest, RestRequest};

use super::{Slot, Target};
use crate::collect::{Reader, failure};
use crate::error::{Failure, Result};
use crate::sha1;
use crate::state::{actions, open, target, write_json};

const MAX_BODY: usize = 65_536;

struct Intent {
  status: String,
  marker: String,
  body: String,
  post_path: String,
  list_path: String,
  kind: String,
  root_comment_id: Option<u64>,
}

impl Intent {
  fn new(repo: &str, number: u64, to: &Target<'_>, body: &str, now: &str) -> Result<Self> {
    let key = to.key();
    let nonce = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .map_or(0, |elapsed| elapsed.as_nanos());
    let seed = format!(
      "{repo}\0{number}\0{key}\0{body}\0{now}\0{}\0{nonce}",
      std::process::id()
    );
    let marker = format!("<!-- toolu-reply:{} -->", sha1::hex(seed.as_bytes()));
    let body = format!("{body}\n\n{marker}");
    if body.len() > MAX_BODY {
      return Err(Failure::usage(
        "reply body with delivery marker exceeds the 65536-byte limit",
      ));
    }
    let (post_path, list_path, root_comment_id) = match to {
      Target::Thread { root, .. } => (
        format!("/repos/{repo}/pulls/{number}/comments/{root}/replies"),
        format!("/repos/{repo}/pulls/{number}/comments?per_page=100"),
        Some(*root),
      ),
      Target::Conversation(_) | Target::Review(_) => (
        format!("/repos/{repo}/issues/{number}/comments"),
        format!("/repos/{repo}/issues/{number}/comments?per_page=100"),
        None,
      ),
    };
    Ok(Self {
      status: "prepared".into(),
      marker,
      body,
      post_path,
      list_path,
      kind: to.kind().into(),
      root_comment_id,
    })
  }

  fn read(value: &Value) -> Result<Self> {
    let field = |name: &str| {
      value
        .get(name)
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| malformed(name))
    };
    let status = field("status")?;
    if !matches!(status.as_str(), "prepared" | "sending") {
      return Err(malformed("status"));
    }
    let kind = field("kind")?;
    let root_comment_id = value.get("rootCommentId").and_then(Value::as_u64);
    if kind == "thread" && root_comment_id.is_none() {
      return Err(malformed("rootCommentId"));
    }
    Ok(Self {
      status,
      marker: field("marker")?,
      body: field("body")?,
      post_path: field("postPath")?,
      list_path: field("listPath")?,
      kind,
      root_comment_id,
    })
  }

  fn value(&self) -> Value {
    json!({
      "status": self.status, "marker": self.marker, "body": self.body,
      "postPath": self.post_path, "listPath": self.list_path,
      "kind": self.kind, "rootCommentId": self.root_comment_id,
    })
  }
}

fn malformed(field: &str) -> Failure {
  Failure::new(
    "state_malformed",
    format!("reply intent has no valid {field}"),
  )
  .with("source", "state")
}

fn save_intent(slot: &Slot<'_>, state: &mut Value, key: &str, intent: &Intent) -> Result<()> {
  actions(state, "replyIntents")?.insert(key.to_owned(), intent.value());
  write_json(slot.path, state, "state")
}

fn uncertain(slot: &Slot<'_>, key: &str, intent: &Intent, reason: &str) -> Failure {
  Failure::new(
    "reply_uncertain",
    format!("reply to {key} may have been posted; inspect GitHub and retry to reconcile"),
  )
  .with("key", key)
  .with("marker", intent.marker.clone())
  .with("stateFile", slot.path.display().to_string())
  .with("reason", reason)
}

fn receipt(
  slot: &Slot<'_>,
  state: &mut Value,
  key: &str,
  intent: &Intent,
  posted: &Value,
) -> Result<Value> {
  let Some(id) = posted.get("id").and_then(Value::as_u64) else {
    return Err(uncertain(
      slot,
      key,
      intent,
      "remote comment has no numeric id",
    ));
  };
  let url = posted.get("html_url").cloned().unwrap_or(Value::Null);
  let head = target(state).2;
  let record = json!({
    "commentId": id, "url": url, "at": slot.now, "headSha": head, "kind": intent.kind,
  });
  actions(state, "replied")?.insert(key.to_owned(), record);
  actions(state, "replyIntents")?.remove(key);
  write_json(slot.path, state, "state")?;
  Ok(json!({"ok": true, "key": key, "commentId": id, "url": url}))
}

fn reconcile(
  client: &Client,
  slot: &Slot<'_>,
  state: &mut Value,
  key: &str,
  intent: &Intent,
) -> Result<Value> {
  let pages = Reader::new(client)
    .rest_pages("reply_reconcile", &intent.list_path)
    .map_err(|err| uncertain(slot, key, intent, &err.message))?;
  let mut found = Vec::new();
  for page in pages {
    let comments = page
      .as_array()
      .ok_or_else(|| uncertain(slot, key, intent, "comment list is not an array"))?;
    for comment in comments {
      let root_matches = intent
        .root_comment_id
        .is_none_or(|root| comment.get("in_reply_to_id").and_then(Value::as_u64) == Some(root));
      if root_matches
        && comment
          .get("body")
          .and_then(Value::as_str)
          .is_some_and(|body| body.contains(&intent.marker))
      {
        found.push(comment.clone());
      }
    }
  }
  match found.as_slice() {
    [posted] => receipt(slot, state, key, intent, posted),
    [] => Err(uncertain(slot, key, intent, "no matching remote comment")),
    _ => Err(uncertain(
      slot,
      key,
      intent,
      "multiple matching remote comments",
    )),
  }
}

fn definitely_unsent(err: &Error) -> bool {
  matches!(
    err,
    Error::Config(_)
      | Error::Token(_)
      | Error::Unauthorized
      | Error::RateLimited { .. }
      | Error::Status {
        status: 400..=499,
        ..
      }
  )
}

fn send(client: &Client, intent: &Intent) -> std::result::Result<Value, Error> {
  let payload = json!({"body": intent.body});
  let reply = client.rest(&RestRequest {
    method: Method::Post,
    path: &intent.post_path,
    body: Some(&payload),
    etag: None,
  })?;
  let Rest::Fresh(fresh) = reply.data else {
    return Err(Error::Decode("reply answered 304".into()));
  };
  fresh.json()
}

fn send_prepared(
  client: &Client,
  slot: &Slot<'_>,
  state: &mut Value,
  key: &str,
  mut intent: Intent,
) -> Result<Value> {
  intent.status = "sending".into();
  save_intent(slot, state, key, &intent)?;
  match send(client, &intent) {
    Ok(posted) if posted.get("id").and_then(Value::as_u64).is_some() => {
      receipt(slot, state, key, &intent, &posted)
    }
    Err(err) if definitely_unsent(&err) => {
      intent.status = "prepared".into();
      save_intent(slot, state, key, &intent)?;
      Err(failure("reply", &err))
    }
    Ok(_) | Err(_) => reconcile(client, slot, state, key, &intent),
  }
}

/// Post one target-scoped reply, or reconcile a prior uncertain send.
///
/// # Errors
/// `duplicate_reply` for a confirmed reply, `reply_uncertain` when remote
/// evidence is inconclusive, or state/GitHub failures before delivery.
pub(crate) fn reply(
  client: &Client,
  slot: &Slot<'_>,
  to: &Target<'_>,
  body: &str,
) -> Result<Value> {
  let (_lock, mut state) = open(slot.path, slot.env)?;
  let key = to.key();
  if let Some(recorded) = actions(&mut state, "replied")?
    .get(&key)
    .filter(|value| !value.is_null())
  {
    return Err(
      Failure::new(
        "duplicate_reply",
        format!("a reply to {key} is already recorded; not posting again"),
      )
      .with("key", key)
      .with("recorded", recorded.clone()),
    );
  }
  let existing = actions(&mut state, "replyIntents")?.get(&key).cloned();
  let intent = if let Some(value) = existing {
    Intent::read(&value)?
  } else {
    let (repo, number, _) = target(&state);
    let intent = Intent::new(repo, number, to, body, slot.now)?;
    save_intent(slot, &mut state, &key, &intent)?;
    intent
  };
  if intent.status == "sending" {
    reconcile(client, slot, &mut state, &key, &intent)
  } else {
    send_prepared(client, slot, &mut state, &key, intent)
  }
}

#[cfg(test)]
#[path = "tests/reply_test.rs"]
mod tests;

#[cfg(test)]
#[path = "tests/reply_recovery_test.rs"]
mod recovery_tests;
