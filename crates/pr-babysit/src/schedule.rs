//! The fixed check schedule: one check every 180 s, anchored on the first
//! scheduled check, so a slow tick never drifts the slots that follow.

use std::time::{Duration, UNIX_EPOCH};

use serde_json::{Value, json};
use toolu_state::time::iso_seconds;

use crate::reduce::Reduced;

/// Seconds between scheduled checks.
pub(crate) const PERIOD: u64 = 180;

/// `seconds` since the epoch as `YYYY-MM-DDTHH:MM:SSZ`.
pub(crate) fn iso(seconds: u64) -> String {
  iso_seconds(UNIX_EPOCH + Duration::from_secs(seconds))
}

/// The schedule after a check at `now`: the previous anchor (or `now`), and
/// the first whole slot after `now`.
pub(crate) fn advance(previous: &Value, now: u64) -> Value {
  let anchor = previous
    .get("anchor")
    .and_then(Value::as_u64)
    .filter(|anchor| *anchor <= now)
    .unwrap_or(now);
  let slots = now.saturating_sub(anchor) / PERIOD + 1;
  json!({"anchor": anchor, "nextCheckAt": anchor + slots * PERIOD})
}

/// Move the state's schedule past `now` when `moves`, and report its next
/// check in the result either way.
pub(crate) fn apply(reduced: &mut Reduced, now: u64, moves: bool) {
  if moves {
    let next = advance(reduced.state.get("schedule").unwrap_or(&Value::Null), now);
    if let Some(state) = reduced.state.as_object_mut() {
      state.insert("schedule".into(), next);
    }
  }
  let next = reduced
    .state
    .pointer("/schedule/nextCheckAt")
    .and_then(Value::as_u64)
    .map_or(Value::Null, |at| Value::from(iso(at)));
  if let Some(result) = reduced.result.as_object_mut() {
    result.insert("nextCheckAt".into(), next);
  }
}

#[cfg(test)]
#[path = "tests/schedule_test.rs"]
mod tests;
