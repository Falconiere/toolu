//! `wait`: run scheduled ticks in process until one needs the agent — success,
//! escalation, attention, or an active fixer — or the deadline passes, which
//! prints the waiting document. A slow or late wake runs one check, never a
//! burst, because each tick schedules the first slot after its own time.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};
use toolu_state::time::parse_iso;

use crate::error::Result;
use crate::schedule::iso;

/// Seconds a Claude `wait` runs: one background Bash call.
pub(crate) const CLAUDE_SECONDS: u64 = 1800;
/// Seconds any other host's `wait` runs: under a foreground tool timeout.
pub(crate) const OTHER_SECONDS: u64 = 55;

/// The default `--max-seconds` for `host`.
pub(crate) fn default_seconds(host: &str) -> u64 {
  if host == "claude" {
    CLAUDE_SECONDS
  } else {
    OTHER_SECONDS
  }
}

/// Time as Unix seconds, and sleeping until a time.
pub(crate) trait Clock {
  /// Now, in Unix seconds.
  fn now(&self) -> u64;
  /// Return at `at` or later.
  fn sleep_until(&self, at: u64);
}

/// The wall clock.
pub(crate) struct SystemClock;

impl Clock for SystemClock {
  fn now(&self) -> u64 {
    SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .map_or(0, |since| since.as_secs())
  }

  fn sleep_until(&self, at: u64) {
    loop {
      let left = at.saturating_sub(self.now());
      if left == 0 {
        return;
      }
      std::thread::sleep(Duration::from_secs(left.min(60)));
    }
  }
}

/// What a wait runs against and reports.
pub(crate) struct Plan<'a> {
  /// The longest the wait runs, in seconds.
  pub(crate) max_seconds: u64,
  /// The state's next scheduled check, when it has one.
  pub(crate) next_check: Option<u64>,
  /// The slot name.
  pub(crate) slot: &'a str,
  /// The state file.
  pub(crate) state_path: &'a str,
  /// The result file.
  pub(crate) result_path: &'a str,
}

/// A tick result the agent must act on.
pub(crate) fn needs_agent(result: &Value) -> bool {
  let decision = result.get("decision").and_then(Value::as_str);
  let attention = result.pointer("/attention/needed") == Some(&Value::Bool(true));
  let fixer = result
    .get("reasons")
    .and_then(Value::as_array)
    .is_some_and(|reasons| {
      reasons
        .iter()
        .any(|reason| reason.get("code").and_then(Value::as_str) == Some("fixer_running"))
    });
  matches!(decision, Some("success" | "escalate")) || attention || fixer
}

/// Run `check` at each scheduled time until a result needs the agent or the
/// plan's deadline passes.
///
/// # Errors
/// The first failed check.
pub(crate) fn wait(
  clock: &dyn Clock,
  plan: &Plan<'_>,
  check: &mut dyn FnMut(u64) -> Result<Value>,
) -> Result<Value> {
  let start = clock.now();
  let deadline = start.saturating_add(plan.max_seconds);
  let mut due = plan.next_check.unwrap_or(start).max(start);
  loop {
    if due > deadline {
      clock.sleep_until(deadline);
      return Ok(json!({
        "version": 1,
        "state": "waiting",
        "slot": plan.slot,
        "nextCheckAt": iso(due),
        "statePath": plan.state_path,
        "resultPath": plan.result_path,
      }));
    }
    clock.sleep_until(due);
    let now = clock.now();
    let result = check(now)?;
    if needs_agent(&result) {
      return Ok(result);
    }
    due = result
      .get("nextCheckAt")
      .and_then(Value::as_str)
      .and_then(parse_iso)
      .and_then(|at| at.duration_since(UNIX_EPOCH).ok())
      .map_or(now + crate::schedule::PERIOD, |since| since.as_secs())
      .max(now + 1);
  }
}

#[cfg(test)]
#[path = "tests/wait_test.rs"]
mod tests;
