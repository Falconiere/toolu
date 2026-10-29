//! The persisted identity and original body of one reply attempt.

use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

use super::super::Target;
use crate::error::{Failure, Result};
use crate::sha1;

const MAX_BODY: usize = 65_536;

/// One reply's durable body, route and delivery marker.
pub(super) struct Intent {
  pub(super) status: String,
  pub(super) marker: String,
  pub(super) body: String,
  pub(super) post_path: String,
  pub(super) list_path: String,
  pub(super) kind: String,
  pub(super) root_comment_id: Option<u64>,
}

impl Intent {
  /// Build an unsent intent for the requested target.
  pub(super) fn new(
    repo: &str,
    number: u64,
    to: &Target<'_>,
    body: &str,
    now: &str,
  ) -> Result<Self> {
    if body.is_empty() {
      return Err(Failure::usage("reply body is empty"));
    }
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

  /// Decode an existing intent without guessing missing fields.
  pub(super) fn read(value: &Value) -> Result<Self> {
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

  /// Encode the intent for atomic state persistence.
  pub(super) fn value(&self) -> Value {
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

/// Refuse a retry that changes the target of a persisted intent.
pub(super) fn validate_target(key: &str, intent: &Intent, to: &Target<'_>) -> Result<()> {
  let requested_root = match to {
    Target::Thread { root, .. } => Some(*root),
    Target::Conversation(_) | Target::Review(_) => None,
  };
  if intent.kind != to.kind() || intent.root_comment_id != requested_root {
    return Err(
      Failure::new(
        "slot_mismatch",
        format!("reply intent for {key} does not match the requested target"),
      )
      .with("key", key)
      .with("persistedRootCommentId", intent.root_comment_id)
      .with("requestedRootCommentId", requested_root),
    );
  }
  Ok(())
}
