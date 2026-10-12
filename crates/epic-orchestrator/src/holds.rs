//! Coupling holds: a batch member that overlaps one ahead of it is held
//! (`hold` on its record, a `coupling-hold` judgment naming the partner, a
//! `hold` journal line) until `toolu epic release` clears it; the released
//! issue's next launch skips the check.

use serde_json::{Value, json};

use crate::journal::Record;
use crate::logic::push_attention;
use crate::model::{Step, World};

/// The members ahead of `key` in its batch whose launch was applied or is
/// waiting: the issues it must not overlap. None once it was released, and
/// none for a relaunch or a reroute, which is no new batch member.
pub(crate) fn coupling_ahead(world: &World, key: &str) -> Vec<String> {
  let Some(issue) = world.issues.get(key) else {
    return Vec::new();
  };
  if issue.coupling_released || !issue.stage.is_empty() {
    return Vec::new();
  }
  let Some(epic) = world.epics.get(&issue.epic) else {
    return Vec::new();
  };
  if !epic.batch.iter().any(|member| member.key == key) {
    return Vec::new();
  }
  epic
    .batch
    .iter()
    .take_while(|member| member.key != key)
    .filter(|member| matches!(member.outcome.as_deref(), Some("applied" | "wait")))
    .map(|member| member.key.clone())
    .collect()
}

/// Hold `key` behind `with`.
pub(crate) fn hold(
  world: &mut World,
  key: &str,
  with: &str,
  score: &Value,
  source: &str,
) -> Vec<Step> {
  let at = toolu_state::time::iso_seconds(
    std::time::UNIX_EPOCH + std::time::Duration::from_millis(world.now_ms),
  );
  if let Some(issue) = world.issues.get_mut(key) {
    issue.pending = None;
    issue.hold =
      Some(json!({"kind": "coupling", "with": with, "score": score, "source": source, "at": at}));
  }
  crate::refill::set_outcome(world, key, "hold");
  world
    .outbox
    .push_back(Step::Journal(Record::new("action", "hold", key, "", with)));
  push_attention(
    world,
    "coupling-hold",
    key,
    &format!("held behind {with} ({source} {score})"),
  );
  world.outbox.push_back(Step::Issue {
    key: key.to_owned(),
  });
  world.outbox.drain(..).collect()
}

/// Clear `key`'s hold so its next launch skips the check.
///
/// # Errors
/// The issue is unknown or not held.
pub(crate) fn release(world: &mut World, key: &str) -> Result<(), String> {
  let issue = world
    .issues
    .get_mut(key)
    .ok_or_else(|| format!("no registered issue {key}"))?;
  let hold = issue
    .hold
    .take()
    .ok_or_else(|| format!("{key} is not held"))?;
  issue.coupling_released = true;
  issue.retry_at_ms = 0;
  let with = hold
    .get("with")
    .and_then(Value::as_str)
    .unwrap_or("")
    .to_owned();
  world.outbox.push_back(Step::Journal(Record::new(
    "action", "release", key, "", &with,
  )));
  world.outbox.push_back(Step::Issue {
    key: key.to_owned(),
  });
  Ok(())
}

#[cfg(test)]
#[path = "tests/holds_test.rs"]
mod tests;
