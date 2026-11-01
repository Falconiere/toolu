//! Commit the sweep that follows a closed epic: its outcome is one journal
//! line (`gc-swept`) and never a judgment. A failed sweep leaves nothing to
//! retry; the next epic, or `toolu gc`, covers it.

use serde_json::Value;

use crate::model::{Step, World};
use crate::refresh::epic_line;

/// The gc effect returned `doc`.
pub(crate) fn commit(world: &mut World, epic: &str, doc: &Value) -> Vec<Step> {
  let note = match doc.get("outcome").and_then(Value::as_str) {
    Some("applied") => doc
      .get("swept")
      .map_or_else(|| "applied".to_owned(), toolu_engine::gc::summary),
    _ => format!(
      "failed: {}",
      doc
        .get("note")
        .and_then(Value::as_str)
        .unwrap_or("gc failed")
    ),
  };
  if let Some(pending) = world
    .epics
    .get_mut(epic)
    .and_then(|found| found.pending.as_mut())
  {
    pending.phase = 2;
    pending.recovered = false;
  }
  world
    .outbox
    .push_back(epic_line("action", "gc-swept", epic, "", &note));
  world.outbox.drain(..).collect()
}

#[cfg(test)]
#[path = "tests/epic_gc_test.rs"]
mod tests;
