//! The documents one tick writes: the comparison fields, the next state and
//! the result.

use serde_json::{Value, json};

use super::Tick;
use super::inputs::Bot;
use crate::json::{array, or, text};

fn field(value: &Value, key: &str) -> Value {
  value.get(key).cloned().unwrap_or(Value::Null)
}

/// The fields whose change makes a tick `changed`.
pub(super) fn comparison(
  pr: &Value,
  ci: &str,
  unresolved: usize,
  head: &str,
  bot: &Bot,
) -> [(&'static str, Value); 8] {
  [
    ("ciStatus", Value::from(ci)),
    ("reviewDecision", field(pr, "reviewDecision")),
    ("mergeable", field(pr, "mergeable")),
    ("unresolvedThreads", Value::from(unresolved)),
    ("headSha", Value::from(head)),
    ("botVerdict", Value::from(bot.label.as_str())),
    ("botState", Value::from(bot.state.as_str())),
    ("botFindingKeys", Value::from(bot.keys.clone())),
  ]
}

fn comment_field(bot: &Bot, key: &str) -> Value {
  or(bot.comment.get(key), Value::Null)
}

fn head(tick: &Tick<'_>) -> Value {
  tick
    .snapshot
    .pointer("/head/sha")
    .cloned()
    .unwrap_or(Value::Null)
}

fn previous_or(tick: &Tick<'_>, key: &str, fallback: Value) -> Value {
  or(tick.previous.and_then(|prev| prev.get(key)), fallback)
}

/// The next version-2 state.
pub(super) fn state(tick: &Tick<'_>) -> Value {
  let snapshot = tick.snapshot;
  let pr = snapshot.get("pr").unwrap_or(&Value::Null);
  let repo = text(snapshot, "repo");
  let number = field(snapshot, "number");
  json!({
    "version": 2,
    "slot": crate::state::slot(repo, number.as_u64().unwrap_or(0)),
    "repo": repo,
    "number": number,
    "lastUpdate": tick.now,
    "totalTicks": tick.memory.ticks + 1,
    "status": previous_or(tick, "status", json!("active")),
    "worktree": previous_or(tick, "worktree", Value::Null),
    "fixer": tick.memory.fixer,
    "herdrWorktree": previous_or(tick, "herdrWorktree", Value::Null),
    "hostCooldowns": previous_or(tick, "hostCooldowns", json!({})),
    "schedule": previous_or(tick, "schedule", Value::Null),
    "attention": {"fingerprint": field(&tick.attention, "fingerprint")},
    "pr": {
      "key": format!("{repo}#{number}"),
      "ciStatus": tick.ci,
      "reviewDecision": field(pr, "reviewDecision"),
      "mergeable": field(pr, "mergeable"),
      "unresolvedThreads": tick.threads.unresolved,
      "headSha": head(tick),
      "fixAttempts": tick.memory.fix_attempts,
      "botVerdict": tick.bot.label,
      "botState": tick.bot.state,
      "botCommentId": comment_field(&tick.bot, "id"),
      "botCommentUpdatedAt": comment_field(&tick.bot, "updatedAt"),
      "botFindingKeys": tick.bot.keys,
      "lastRoundFindingKeys": tick.memory.last_round_keys,
      "lastRoundHadRejection": tick.memory.had_rejection,
      "recurrenceStreak": tick.streak,
      "unresolvedAfterClearance": tick.threads.unresolved,
      "lastError": null,
    },
    "actions": tick.memory.actions,
    "lastGoodSnapshot": tick.paths.snapshot,
  })
}

fn pr_doc(tick: &Tick<'_>) -> Value {
  let pr = tick.snapshot.get("pr").unwrap_or(&Value::Null);
  json!({
    "number": field(pr, "number"),
    "url": field(pr, "url"),
    "head": head(tick),
    "branch": field(pr, "headRefName"),
    "base": field(pr, "baseRefName"),
    "author": field(pr, "author"),
    "state": field(pr, "state"),
    "mergeable": field(pr, "mergeable"),
    "reviewDecision": field(pr, "reviewDecision"),
  })
}

fn verdict_doc(bot: &Bot) -> Value {
  json!({
    "state": bot.state,
    "verdict": bot.label,
    "findingsCount": bot.keys.len(),
    "findingKeys": bot.keys,
    "mustFix": or(bot.verdict.get("must_fix"), json!([])),
    "commentUrl": comment_field(bot, "url"),
    "commentId": comment_field(bot, "id"),
    "degraded": bot.degraded.is_some(),
    "degradedReason": bot.degraded,
    "sameRunAsLastTick": bot.same_run,
  })
}

fn threads_doc(tick: &Tick<'_>) -> Value {
  json!({
    "total": array(tick.snapshot.get("threads")).len(),
    "unresolved": tick.threads.unresolved,
    "actionable": tick.threads.actionable,
    "fixing": tick.threads.fixing,
    "staleUnresolved": tick.threads.stale,
    "skippedOutdated": tick.threads.skipped,
    "flaggedInjection": tick.threads.flagged,
  })
}

/// The version-1 result.
pub(super) fn result(tick: &Tick<'_>) -> Value {
  let snapshot = tick.snapshot;
  let number = field(snapshot, "number").as_u64().unwrap_or(0);
  json!({
    "version": 1,
    "slot": crate::state::slot(text(snapshot, "repo"), number),
    "changed": tick.changed,
    "decision": tick.decision,
    "reasons": tick.reasons,
    "attention": tick.attention,
    "pr": pr_doc(tick),
    "ci": {"status": tick.ci, "checks": tick.checks},
    "verdict": verdict_doc(&tick.bot),
    "threads": threads_doc(tick),
    "conversation": {"actionable": tick.conversation.0, "fixing": tick.conversation.1},
    "reviews": {"actionable": tick.reviews.0, "fixing": tick.reviews.1},
    "fixer": tick.memory.fixer,
    "recurrence": {
      "streak": tick.streak,
      "lastRoundHadRejection": tick.memory.had_rejection,
      "recurringKeys": tick.recurring,
      "fixAttempts": tick.memory.fix_attempts,
    },
    "nextCheckAt": null,
    "errors": [],
    "snapshotPath": tick.paths.snapshot,
    "statePath": tick.paths.state,
    "resultPath": tick.paths.result,
  })
}

#[cfg(test)]
#[path = "tests/docs_test.rs"]
mod tests;
