//! Close a complete epic once. The close is armed after a refresh shows every
//! sub-issue done and no issue occupies a slot; it posts the summary, closes
//! the epic, journals `epic-closed` and raises `epic-complete`. A close
//! recovered after a restart only asks the tracker: closed is done, open is
//! `epic close uncertain`. A failed or uncertain close is never tried again.

use serde_json::Value;

use crate::logic::token;
use crate::model::{Action, Step, World, fresh};
use crate::refresh::{epic_attention, epic_line};

/// Arm the close of `epic` when it is complete, open and idle.
pub(crate) fn arm(world: &mut World, epic: &str) {
  let Some(found) = world.epics.get(epic) else {
    return;
  };
  let busy = world
    .issues
    .values()
    .any(|issue| issue.epic == epic && crate::refill::occupies(issue));
  if !found.complete || found.closed || found.close_blocked || found.pending.is_some() || busy {
    return;
  }
  let token = token(world);
  if let Some(found) = world.epics.get_mut(epic) {
    found.pending = Some(fresh(Action::Close, token));
  }
}

/// The close effect returned `doc`.
pub(crate) fn commit(world: &mut World, epic: &str, doc: &Value) -> Vec<Step> {
  let outcome = doc
    .get("outcome")
    .and_then(Value::as_str)
    .unwrap_or("failed");
  match outcome {
    "applied" | "closed" => closed(world, epic, outcome),
    "open" => blocked(world, epic, "uncertain", "epic close uncertain"),
    _ => {
      let note = doc
        .get("note")
        .and_then(Value::as_str)
        .unwrap_or("epic close failed");
      blocked(world, epic, "failed", note)
    }
  }
}

fn closed(world: &mut World, epic: &str, outcome: &str) -> Vec<Step> {
  if let Some(found) = world.epics.get_mut(epic) {
    found.closed = true;
    if let Some(pending) = found.pending.as_mut() {
      pending.phase = 2;
      pending.recovered = false;
    }
  }
  world
    .outbox
    .push_back(epic_line("action", "epic-closed", epic, "", outcome));
  epic_attention(
    world,
    "epic-complete",
    epic,
    "every sub-issue is done; the epic is closed",
  );
  world.outbox.drain(..).collect()
}

/// End the close with a `failed` judgment; the done line keeps it ended.
fn blocked(world: &mut World, epic: &str, how: &str, note: &str) -> Vec<Step> {
  let token = world
    .epics
    .get_mut(epic)
    .and_then(|found| {
      found.close_blocked = true;
      found.pending.take()
    })
    .map(|pending| pending.token)
    .unwrap_or_default();
  epic_attention(world, "failed", epic, note);
  world
    .outbox
    .push_back(epic_line("action", "close-done", epic, &token, how));
  world.outbox.drain(..).collect()
}

#[cfg(test)]
#[path = "tests/epic_close_test.rs"]
mod tests;
