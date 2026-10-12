//! Epic refresh on the state thread. A refresh is due on #447's
//! `EpicChanged`, after a finished cleanup, and every 15 minutes for Jira and
//! Linear epics. Its intent and done lines carry the epic key; a failure keeps
//! the previous graph, stays pending and is retried on the fixed 180 s
//! deadline, with one `refresh-failed` at the third failure in a row. A
//! success reloads the graph and raises one `reopened` for a merged issue
//! whose tracker item was seen closed and is open again.

use serde_json::{Value, json};

use crate::journal::Record;
use crate::logic::{push_attention, token};
use crate::model::{Action, Seen, Step, World, fresh};
use crate::watch::CHECK_MS;

/// Failures in a row that raise `refresh-failed`.
const FAILED_AT: u32 = 3;

/// Arm a refresh of `epic`, due now, unless an epic action is in progress.
pub(crate) fn arm(world: &mut World, epic: &str) {
  if world
    .epics
    .get(epic)
    .is_none_or(|found| found.pending.is_some())
  {
    return;
  }
  let token = token(world);
  let now = world.now_ms;
  if let Some(found) = world.epics.get_mut(epic) {
    found.pending = Some(fresh(Action::Refresh, token));
    found.refresh_at_ms = now;
  }
}

/// Arm the refresh of every epic whose poll is due (Jira and Linear).
pub(crate) fn poll(world: &mut World) {
  let due: Vec<String> = world
    .epics
    .values()
    .filter(|found| found.pending.is_none() && found.refresh_at_ms <= world.now_ms)
    .map(|found| found.key.clone())
    .collect();
  for epic in due {
    arm(world, &epic);
  }
}

/// Whether `name` is the intent or done line of an epic action.
pub(crate) fn epic_record(name: &str) -> bool {
  matches!(
    name,
    "refresh-intent" | "refresh-done" | "close-intent" | "close-done"
  )
}

/// A journal record for `epic`.
pub(crate) fn epic_line(kind: &str, name: &str, epic: &str, token: &str, note: &str) -> Step {
  let mut record = Record::new(kind, name, epic, token, note);
  epic.clone_into(&mut record.epic);
  Step::Journal(record)
}

/// The next step of an epic action that may run now.
pub(crate) fn epic_step(world: &World) -> Option<Step> {
  world.epics.values().find_map(|epic| {
    if world.paused_all || world.paused.contains(&epic.key) {
      return None;
    }
    let pending = epic.pending.as_ref()?;
    let name = pending.action.name();
    match pending.phase {
      0 => Some(epic_line(
        "action",
        &format!("{name}-intent"),
        &epic.key,
        &pending.token,
        "",
      )),
      2 => Some(epic_line(
        "action",
        &format!("{name}-done"),
        &epic.key,
        &pending.token,
        "",
      )),
      _ => {
        let waiting = pending.action == Action::Refresh && world.now_ms < epic.refresh_at_ms;
        let blocked = world.effect_busy || world.draining || waiting;
        (!blocked).then(|| Step::Call {
          key: epic.key.clone(),
          action: pending.action,
          token: pending.token.clone(),
        })
      }
    }
  })
}

/// The intent or done line of `epic`'s action was written.
pub(crate) fn commit_journal(world: &mut World, record: &Record) -> Vec<Step> {
  if record.name.ends_with("-intent") {
    if let Some(pending) = world
      .epics
      .get_mut(&record.key)
      .and_then(|found| found.pending.as_mut())
    {
      pending.phase = 1;
    }
    return Vec::new();
  }
  let action = finish(world, &record.key);
  if action == Some(Action::Refresh) {
    crate::epic_close::arm(world, &record.key);
  }
  world.outbox.drain(..).collect()
}

/// End `epic`'s action and return which one it was.
pub(crate) fn finish(world: &mut World, epic: &str) -> Option<Action> {
  world
    .epics
    .get_mut(epic)
    .and_then(|found| found.pending.take())
    .map(|pending| pending.action)
}

/// The epic action's effect returned `outcome`.
pub(crate) fn commit(world: &mut World, epic: &str, outcome: &str) -> Vec<Step> {
  let doc: Value = serde_json::from_str(outcome)
    .unwrap_or_else(|_err| json!({"outcome": "failed", "note": outcome}));
  let action = world
    .epics
    .get(epic)
    .and_then(|found| found.pending.as_ref())
    .map(|pending| pending.action);
  if action == Some(Action::Close) {
    return crate::epic_close::commit(world, epic, &doc);
  }
  if action != Some(Action::Refresh) {
    return Vec::new();
  }
  let note = doc
    .get("note")
    .and_then(Value::as_str)
    .unwrap_or("refresh failed");
  if doc.get("outcome").and_then(Value::as_str) != Some("applied") {
    return failed(world, epic, note);
  }
  if let Err(err) = crate::snapshot::reload_epic(world, epic) {
    return failed(world, epic, &err);
  }
  let now = world.now_ms;
  crate::attention::settle(world, epic, Some("refresh-failed"), "refresh");
  if let Some(found) = world.epics.get_mut(epic) {
    found.refresh_failures = 0;
    found.refresh_at_ms = found.next_poll(now);
    if let Some(pending) = found.pending.as_mut() {
      pending.phase = 2;
    }
  }
  reopened(world, epic);
  world.outbox.drain(..).collect()
}

/// Keep the previous graph and try again after the fixed deadline.
fn failed(world: &mut World, epic: &str, note: &str) -> Vec<Step> {
  let now = world.now_ms;
  let failures = world.epics.get_mut(epic).map_or(0, |found| {
    found.refresh_failures = found.refresh_failures.saturating_add(1);
    found.refresh_at_ms = now.saturating_add(CHECK_MS);
    found.refresh_failures
  });
  world
    .outbox
    .push_back(epic_line("action", "refresh-error", epic, "", note));
  if failures == FAILED_AT {
    epic_attention(world, "refresh-failed", epic, note);
  }
  world.outbox.drain(..).collect()
}

/// A judgment about `epic` itself; the queue names the epic from its key.
pub(crate) fn epic_attention(world: &mut World, kind: &str, epic: &str, note: &str) {
  push_attention(world, kind, epic, note);
}

/// Note what the refreshed graph shows of each merged issue's tracker item.
fn reopened(world: &mut World, epic: &str) {
  let states: Vec<(String, String)> = world
    .epics
    .get(epic)
    .and_then(|found| found.graph.as_ref())
    .map(|graph| {
      graph
        .issues
        .iter()
        .map(|issue| (issue.key.clone(), issue.state.clone()))
        .collect()
    })
    .unwrap_or_default();
  for (key, state) in states {
    let Some(issue) = world.issues.get_mut(&key) else {
      continue;
    };
    if issue.stage != "merged" {
      continue;
    }
    let seen = match (issue.tracker_seen, state.as_str()) {
      (Seen::Open, "closed") => Seen::Closed,
      (Seen::Closed, "open") => Seen::Reopened,
      (Seen::Open | Seen::Closed | Seen::Reopened, _) => continue,
    };
    issue.tracker_seen = seen;
    world.outbox.push_back(Step::Issue { key: key.clone() });
    if seen == Seen::Reopened {
      let note = "tracker item reopened after its merge; read the reopening comment";
      push_attention(world, "reopened", &key, note);
    }
  }
}

#[cfg(test)]
#[path = "tests/refresh_test.rs"]
mod tests;
