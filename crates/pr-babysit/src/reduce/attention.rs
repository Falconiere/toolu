//! What an agent must do now: the attention codes, a fingerprint of that work,
//! and whether the fingerprint is new since the previous tick. `wait` returns
//! on any attention; the epic engine prompts only on new attention, so one
//! piece of work reaches a worker once.

use serde_json::{Value, json};

use crate::json::{jq_string, text};

/// The work one tick found, already split by owner.
pub(crate) struct Work<'a> {
  /// Escalation reasons.
  pub(crate) escalations: &'a [Value],
  /// `ci.status`.
  pub(crate) ci: &'a str,
  /// `ci.checks`.
  pub(crate) checks: &'a [Value],
  /// The head SHA.
  pub(crate) head: &'a str,
  /// Actionable threads.
  pub(crate) threads: &'a [Value],
  /// Replied but unresolved threads.
  pub(crate) stale: &'a [Value],
  /// Actionable conversation comments.
  pub(crate) conversation: &'a [Value],
  /// Actionable reviews.
  pub(crate) reviews: &'a [Value],
  /// A new verdict run that is complete and not approved with zero findings.
  pub(crate) review_changes: bool,
  /// A new verdict run that reported a provider error.
  pub(crate) provider_error: bool,
  /// `<commentId>@<updatedAt>` of the bot comment.
  pub(crate) bot_run: String,
  /// The slot's fixer record.
  pub(crate) fixer: Option<&'a Value>,
}

fn ids(items: &[Value], prefix: &str, reply: bool) -> Vec<String> {
  items
    .iter()
    .map(|item| {
      let id = item.get("id").map_or_else(String::new, jq_string);
      if reply {
        let to = item.get("inReplyTo").map_or_else(String::new, jq_string);
        format!("{prefix}:{id}@{to}")
      } else {
        format!("{prefix}:{id}")
      }
    })
    .collect()
}

fn fixer_settled(fixer: Option<&Value>) -> Option<String> {
  let fixer = fixer?;
  let status = text(fixer, "status");
  matches!(status, "done" | "failed").then(|| {
    let round = fixer.get("round").map_or_else(String::new, jq_string);
    format!("fixer:{round}:{status}")
  })
}

impl Work<'_> {
  /// Each attention code with the fingerprint parts of its work, in order.
  fn found(&self) -> Vec<(&'static str, Vec<String>)> {
    let mut found = Vec::new();
    if !self.escalations.is_empty() {
      let parts = self.escalations.iter();
      found.push((
        "escalate",
        parts
          .map(|r| format!("escalate:{}", text(r, "code")))
          .collect(),
      ));
    }
    if self.ci == "fail" {
      let failing = self
        .checks
        .iter()
        .filter(|check| text(check, "status") == "fail")
        .map(|check| format!("check:{}", text(check, "name")));
      let parts = std::iter::once(format!("ci:{}", self.head)).chain(failing);
      found.push(("ci_failed", parts.collect()));
    }
    for (code, items, prefix, reply) in [
      ("threads_actionable", self.threads, "thread", true),
      ("threads_stale_unresolved", self.stale, "stale", false),
      (
        "conversation_actionable",
        self.conversation,
        "conversation",
        false,
      ),
      ("reviews_actionable", self.reviews, "review", false),
    ] {
      if !items.is_empty() {
        found.push((code, ids(items, prefix, reply)));
      }
    }
    for (code, on) in [
      ("review_changes", self.review_changes),
      ("provider_error", self.provider_error),
    ] {
      if on {
        found.push((code, vec![format!("{code}:{}", self.bot_run)]));
      }
    }
    if let Some(settled) = fixer_settled(self.fixer) {
      found.push(("fixer_done", vec![settled]));
    }
    found
  }

  /// `{needed, new, codes, fingerprint}` against the previous fingerprint.
  pub(crate) fn attention(&self, previous: Option<&str>) -> Value {
    let found = self.found();
    let codes: Vec<&str> = found.iter().map(|(code, _)| *code).collect();
    let parts: Vec<String> = found.into_iter().flat_map(|(_, parts)| parts).collect();
    let fingerprint = (!codes.is_empty()).then(|| crate::sha1::hex(parts.join("\n").as_bytes()));
    let new = fingerprint
      .as_deref()
      .is_some_and(|print| previous != Some(print));
    json!({
      "needed": !codes.is_empty(),
      "new": new,
      "codes": codes,
      "fingerprint": fingerprint,
    })
  }
}

#[cfg(test)]
#[path = "tests/attention_test.rs"]
mod tests;
