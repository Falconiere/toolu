//! What the reducer reads: the checks, the bot verdict of the snapshot, and the
//! memory carried in the previous state.

use serde_json::{Value, json};

use crate::json::{array, jq_string, nil, or, text};

fn check_state(check: &Value) -> &'static str {
  let state = text(check, "state");
  if text(check, "__typename") == "StatusContext" {
    return match state {
      "SUCCESS" => "pass",
      "PENDING" | "EXPECTED" => "pending",
      _ => "fail",
    };
  }
  if text(check, "status") != "COMPLETED" {
    "pending"
  } else if matches!(text(check, "conclusion"), "SUCCESS" | "NEUTRAL" | "SKIPPED") {
    "pass"
  } else {
    "fail"
  }
}

/// `{name, status, url}` for every check of the rollup.
pub(crate) fn checks(pr: &Value) -> Vec<Value> {
  array(pr.get("statusCheckRollup"))
    .iter()
    .map(|check| {
      json!({
        "name": or(check.get("name"), or(check.get("context"), json!("unknown"))),
        "status": check_state(check),
        "url": or(check.get("detailsUrl"), or(check.get("targetUrl"), Value::Null)),
      })
    })
    .collect()
}

/// `fail` when any check failed, else `pending` when any is pending or none
/// reported, else `pass`.
pub(crate) fn ci_status(checks: &[Value]) -> &'static str {
  let has = |status: &str| checks.iter().any(|check| text(check, "status") == status);
  if has("fail") {
    "fail"
  } else if checks.is_empty() || has("pending") {
    "pending"
  } else {
    "pass"
  }
}

/// The bot comment and its verdict.
pub(crate) struct Bot {
  /// The comment, or `null`.
  pub(crate) comment: Value,
  /// The parsed verdict, or `{}`.
  pub(crate) verdict: Value,
  /// `absent`, `unknown`, `in_progress`, `complete` or `provider_error`.
  pub(crate) state: String,
  /// `approved`, `changes` or `none`.
  pub(crate) label: String,
  /// The finding keys.
  pub(crate) keys: Vec<Value>,
  /// `review_absent` or `review_unknown_format` when unreadable.
  pub(crate) degraded: Option<&'static str>,
  /// The same comment id and `updatedAt` as the previous tick.
  pub(crate) same_run: bool,
}

/// Read the bot comment of `snapshot` against the previous state.
pub(crate) fn bot(snapshot: &Value, previous: Option<&Value>) -> Bot {
  let verdict = or(snapshot.pointer("/bot/verdict"), json!({}));
  let comment = snapshot
    .pointer("/bot/comment")
    .cloned()
    .unwrap_or(Value::Null);
  let state = nil(verdict.get("state"))
    .and_then(Value::as_str)
    .unwrap_or("absent")
    .to_owned();
  let label = nil(verdict.get("verdict"))
    .and_then(Value::as_str)
    .unwrap_or("none")
    .to_owned();
  let keys = array(verdict.get("findings"))
    .iter()
    .map(|finding| finding.get("key").cloned().unwrap_or(Value::Null))
    .collect();
  let unreadable = state == "absent"
    || state == "unknown"
    || verdict.get("is_review_comment") == Some(&Value::Bool(false));
  let degraded = unreadable.then_some(if state == "absent" {
    "review_absent"
  } else {
    "review_unknown_format"
  });
  let prev_pr = previous.and_then(|prev| prev.get("pr"));
  let field = |key: &str| or(prev_pr.and_then(|pr| pr.get(key)), Value::Null);
  let same_run = previous.is_some()
    && !comment.is_null()
    && field("botCommentId") == comment.get("id").cloned().unwrap_or(Value::Null)
    && field("botCommentUpdatedAt") == comment.get("updatedAt").cloned().unwrap_or(Value::Null);
  Bot {
    comment,
    verdict,
    state,
    label,
    keys,
    degraded,
    same_run,
  }
}

/// What the previous state carries into this tick.
pub(crate) struct Memory {
  /// `replied`, `resolved` and `flagged`.
  pub(crate) actions: Value,
  /// The fixer record, or `null`.
  pub(crate) fixer: Value,
  /// Ids an active (running or blocked) fixer owns.
  pub(crate) fixing: Option<Vec<String>>,
  /// Finding keys of the last round.
  pub(crate) last_round_keys: Vec<Value>,
  /// The last round rejected a finding.
  pub(crate) had_rejection: bool,
  /// Recorded fix attempts.
  pub(crate) fix_attempts: u64,
  /// The previous recurrence streak.
  pub(crate) streak: u64,
  /// Ticks so far.
  pub(crate) ticks: u64,
}

/// Read the memory of `previous`.
pub(crate) fn memory(previous: Option<&Value>) -> Memory {
  let empty = json!({"replied": {}, "resolved": {}, "flagged": {}});
  let actions = previous.map_or_else(
    || empty.clone(),
    |prev| or(prev.get("actions"), empty.clone()),
  );
  let fixer = previous.map_or(Value::Null, |prev| or(prev.get("fixer"), Value::Null));
  let fixing = matches!(text(&fixer, "status"), "running" | "blocked")
    .then(|| array(fixer.get("items")).iter().map(jq_string).collect());
  let pr = previous.and_then(|prev| prev.get("pr"));
  let num = |key: &str| {
    pr.and_then(|pr| nil(pr.get(key)))
      .and_then(Value::as_u64)
      .unwrap_or(0)
  };
  Memory {
    actions,
    fixing,
    fixer,
    last_round_keys: array(pr.and_then(|pr| pr.get("lastRoundFindingKeys"))).to_vec(),
    had_rejection: pr
      .and_then(|pr| nil(pr.get("lastRoundHadRejection")))
      .is_some(),
    fix_attempts: num("fixAttempts"),
    streak: num("recurrenceStreak"),
    ticks: previous
      .and_then(|prev| nil(prev.get("totalTicks")))
      .and_then(Value::as_u64)
      .unwrap_or(0),
  }
}

impl Memory {
  /// Whether an active fixer owns `item`.
  pub(crate) fn owns(&self, item: &Value) -> bool {
    self.fixing.as_ref().is_some_and(|ids| {
      item
        .get("id")
        .is_some_and(|id| ids.contains(&jq_string(id)))
    })
  }
}

#[cfg(test)]
#[path = "tests/inputs_test.rs"]
mod tests;
