//! Commit an effect outcome into journal steps.

use serde_json::{Value, json};

use crate::journal::Record;
use crate::logic::push_attention;
use crate::model::{Action, Pending, Seen, Step, World};

/// The effect returned `outcome`.
pub(crate) fn commit_effect(world: &mut World, key: &str, outcome: &str) -> Vec<Step> {
  if !world.issues.contains_key(key) && world.epics.contains_key(key) {
    return crate::refresh::commit(world, key, outcome);
  }
  let action = pending_action(world, key);
  let recovered = pending_recovered(world, key);
  if recovered && outcome == "deferred" {
    return defer(world, key, action);
  }
  if recovered {
    return commit_reconcile(world, key, outcome);
  }
  if outcome == "deferred" {
    return defer(world, key, action);
  }
  // Any other launch or cleanup outcome, a panic's text included, is read
  // as its outcome document; so is the in-process merge-queue step's.
  let plain = matches!(outcome, "applied" | "already");
  if action == Some(Action::Merge)
    && !plain
    && let Some(steps) = crate::commit_queue::commit(world, key, outcome)
  {
    return steps;
  }
  match action {
    Some(Action::Launch) if !plain => commit_launch(world, key, outcome),
    Some(Action::Cleanup) if !plain => commit_cleanup(world, key, outcome),
    _ if plain => applied(world, key, action),
    _ => fail_pending(world, key, "failed", outcome),
  }
}

/// The in-process launch's outcome document.
fn commit_launch(world: &mut World, key: &str, outcome: &str) -> Vec<Step> {
  let doc: Value = serde_json::from_str(outcome)
    .unwrap_or_else(|_err| json!({"outcome": "failed", "note": outcome}));
  let text = |name: &str| {
    doc
      .get(name)
      .and_then(Value::as_str)
      .unwrap_or("")
      .to_owned()
  };
  if let Some(reason) = doc.get("fallback").and_then(Value::as_str) {
    world.outbox.push_back(Step::Journal(Record::new(
      "action",
      "coupling-fallback",
      key,
      "",
      reason,
    )));
  }
  match text("outcome").as_str() {
    "applied" => launched(world, key, doc.get("kind").and_then(Value::as_str)),
    "hold" => {
      let _closed = close_launch(world, key, "hold");
      let score = doc.get("score").cloned().unwrap_or(Value::Null);
      crate::holds::hold(world, key, &text("with"), &score, &text("source"))
    }
    "wait" => {
      let reroute = close_launch(world, key, "wait").is_some_and(|pending| pending.reroute);
      crate::refill::note_wait(world, key, &text("note"));
      crate::refill::mark_relaunch(world, key, reroute);
      world.outbox.drain(..).collect()
    }
    _ => launch_failed(world, key, &doc),
  }
}

/// End the pending launch without a stage change. Its done line carries
/// `note`, so a restart neither runs it again nor finishes it as applied.
fn close_launch(world: &mut World, key: &str, note: &str) -> Option<Pending> {
  let pending = world.issues.get_mut(key)?.pending.take()?;
  world.outbox.push_back(Step::Journal(Record::new(
    "action",
    "launch-done",
    key,
    &pending.token,
    note,
  )));
  Some(pending)
}

/// A failed launch keeps the record's stage and is not tried again until
/// the worker or a human moves the issue.
fn launch_failed(world: &mut World, key: &str, doc: &Value) -> Vec<Step> {
  crate::refill::set_outcome(world, key, "failed");
  if let Some(issue) = world.issues.get_mut(key) {
    if let Some(stage) = doc.get("stage").and_then(Value::as_str) {
      stage.clone_into(&mut issue.stage);
    }
    issue.deferred = Some("launch".to_owned());
    issue.relaunch = None;
  }
  world.outbox.push_back(Step::Issue {
    key: key.to_owned(),
  });
  let _closed = close_launch(world, key, "failed");
  let note = doc.get("note").and_then(Value::as_str).unwrap_or("");
  fail_pending(world, key, "failed", note)
}

/// The in-process cleanup's outcome document: a finished issue journals its
/// settlement (a failed one is a `failed` judgment) and notes that its
/// tracker item is closed; a refusal leaves `cleanup-incomplete`.
fn commit_cleanup(world: &mut World, key: &str, outcome: &str) -> Vec<Step> {
  let doc: Value = serde_json::from_str(outcome)
    .unwrap_or_else(|_err| json!({"outcome": "failed", "note": outcome}));
  if doc.get("outcome").and_then(Value::as_str) != Some("applied") {
    let note = doc.get("note").and_then(Value::as_str).unwrap_or(outcome);
    if let Some(issue) = world.issues.get_mut(key) {
      "cleanup-incomplete".clone_into(&mut issue.stage);
    }
    world.outbox.push_back(Step::Issue {
      key: key.to_owned(),
    });
    return fail_pending(world, key, "cleanup-incomplete", note);
  }
  if let Some(settled) = doc.get("settle") {
    let note = settled.to_string();
    world.outbox.push_back(Step::Journal(Record::new(
      "action", "settle", key, "", &note,
    )));
  }
  if let Some(err) = doc.get("settle_error").and_then(Value::as_str) {
    push_attention(world, "failed", key, &format!("settle: {err}"));
  }
  if doc.get("tracker_closed") == Some(&Value::Bool(true))
    && let Some(issue) = world.issues.get_mut(key)
    && issue.tracker_seen == Seen::Open
  {
    issue.tracker_seen = Seen::Closed;
  }
  crate::attention::settle(world, key, Some("cleanup-incomplete"), "cleanup");
  let mut steps: Vec<Step> = world.outbox.drain(..).collect();
  steps.extend(applied(world, key, Some(Action::Cleanup)));
  steps
}

/// A launch applied on host `kind`: end its wait streak and its released state.
fn launched(world: &mut World, key: &str, kind: Option<&str>) -> Vec<Step> {
  crate::refill::end_wait(world, key);
  crate::refill::set_outcome(world, key, "applied");
  if let Some(issue) = world.issues.get_mut(key) {
    issue.coupling_released = false;
    issue.relaunch = None;
    if let Some(kind) = kind {
      kind.clone_into(&mut issue.kind);
    }
  }
  let mut steps: Vec<Step> = world.outbox.drain(..).collect();
  steps.extend(applied(world, key, Some(Action::Launch)));
  steps
}

fn pending_action(world: &World, key: &str) -> Option<Action> {
  world
    .issues
    .get(key)
    .and_then(|issue| issue.pending.as_ref())
    .map(|pending| pending.action)
}

fn pending_recovered(world: &World, key: &str) -> bool {
  world
    .issues
    .get(key)
    .and_then(|issue| issue.pending.as_ref())
    .is_some_and(|pending| pending.recovered)
}

pub(crate) fn applied(world: &mut World, key: &str, action: Option<Action>) -> Vec<Step> {
  if let Some(issue) = world.issues.get_mut(key) {
    if let Some(pending) = issue.pending.as_mut() {
      pending.phase = 2;
    }
    if action == Some(Action::Merge) {
      "cleaning".clone_into(&mut issue.stage);
    }
    if action == Some(Action::Launch) {
      "running".clone_into(&mut issue.stage);
    }
  }
  if action == Some(Action::Merge) {
    crate::watch::after_merge(world, key);
    // The merged issue left its queue: the next head goes.
    crate::logic::arm_ready(world);
  }
  vec![Step::Issue {
    key: key.to_owned(),
  }]
}

fn defer(world: &mut World, key: &str, action: Option<Action>) -> Vec<Step> {
  if let Some(issue) = world.issues.get_mut(key) {
    issue.deferred = action.map(Action::name).map(str::to_owned);
    issue.pending = None;
  }
  if action == Some(Action::Merge) {
    crate::merge_bridge::merge_ready(world, key);
  }
  world.outbox.push_back(Step::Journal(Record::new(
    "action",
    "deferred",
    key,
    "",
    action.map_or("", Action::name),
  )));
  world.outbox.drain(..).collect()
}

fn fail_pending(world: &mut World, key: &str, kind: &str, note: &str) -> Vec<Step> {
  if let Some(issue) = world.issues.get_mut(key) {
    issue.pending = None;
  }
  push_attention(world, kind, key, note);
  world.outbox.drain(..).collect()
}

fn commit_reconcile(world: &mut World, key: &str, outcome: &str) -> Vec<Step> {
  match outcome {
    "merged" | "already" => mark_recovered_done(world, key),
    "open" => {
      clear_recovered(world, key);
      Vec::new()
    }
    _ => fail_pending(world, key, "failed", "outcome unknown"),
  }
}

fn mark_recovered_done(world: &mut World, key: &str) -> Vec<Step> {
  if let Some(pending) = world
    .issues
    .get_mut(key)
    .and_then(|issue| issue.pending.as_mut())
  {
    pending.phase = 2;
    pending.recovered = false;
  }
  if let Some(issue) = world.issues.get_mut(key)
    && issue.stage != "merged"
  {
    "cleaning".clone_into(&mut issue.stage);
  }
  crate::watch::after_merge(world, key);
  vec![Step::Issue {
    key: key.to_owned(),
  }]
}

fn clear_recovered(world: &mut World, key: &str) {
  if let Some(pending) = world
    .issues
    .get_mut(key)
    .and_then(|issue| issue.pending.as_mut())
  {
    pending.recovered = false;
  }
}

#[cfg(test)]
#[path = "tests/commit_test.rs"]
mod tests;

#[cfg(test)]
#[path = "tests/commit_launch_test.rs"]
mod launch_tests;
