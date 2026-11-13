//! Drain the engine outbox, batching consecutive journal rows.

use crate::journal::{self, Record};
use crate::logic::commit_journal;
use crate::model::{Step, World};
use crate::server::Engine;

/// Consecutive journal steps at the front share one lock and one JSONL sync
/// before any of their follow-ups run.
///
/// # Errors
/// A step cannot be written.
pub(crate) fn flush(engine: &mut Engine) -> Result<(), String> {
  loop {
    let mut batch = Vec::new();
    while matches!(engine.world.outbox.front(), Some(Step::Journal(_))) {
      let Some(Step::Journal(record)) = engine.world.outbox.pop_front() else {
        break;
      };
      batch.push(stamp(&engine.world, record));
    }
    if !batch.is_empty() {
      let stored = journal::append_batch(
        &engine.paths.journal_dir(),
        &engine.paths.journal_lock(),
        &batch,
        engine.now,
      )?;
      for record in stored {
        let steps = commit_journal(&mut engine.world, &record);
        if !steps.is_empty() {
          journal::batch_event("follow");
        }
        for step in steps {
          let _applied = engine.apply_step(step)?;
        }
      }
      continue;
    }
    let Some(step) = engine.world.outbox.pop_front() else {
      return Ok(());
    };
    let _applied = engine.apply_step(step)?;
  }
}

/// Fill `epic` from the issue or epic map when the record left it empty.
pub(crate) fn stamp(world: &World, mut record: Record) -> Record {
  if record.epic.is_empty() {
    let issue = crate::watch::issue_of(world, &record.key).unwrap_or(&record.key);
    record.epic = world
      .issues
      .get(issue)
      .map(|issue| issue.epic.clone())
      .or_else(|| {
        world
          .epics
          .contains_key(&record.key)
          .then(|| record.key.clone())
      })
      .unwrap_or_default();
  }
  record
}
