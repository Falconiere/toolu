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
      match journal::append_batch(
        &engine.paths.journal_dir(),
        &engine.paths.journal_lock(),
        &batch,
        engine.now,
      ) {
        Ok(stored) => apply_batch_follow_ups(engine, stored)?,
        Err(err) => {
          restore_on_lock_busy(engine, batch, &err);
          return Err(err);
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

fn restore_on_lock_busy(engine: &mut Engine, batch: Vec<Record>, err: &str) {
  // Only the lock wait fails before any line is written. Later write or cache
  // errors may leave durable lines; re-queueing those would append duplicates.
  if !err.starts_with("journal-busy") {
    return;
  }
  for record in batch.into_iter().rev() {
    engine.world.outbox.push_front(Step::Journal(record));
  }
}

/// Run each durable row's `commit_journal` follow-ups in order.
///
/// On an apply error, remaining follow-up steps (including later rows' commits)
/// are pushed back onto the outbox so durable rows are not dropped.
fn apply_batch_follow_ups(engine: &mut Engine, stored: Vec<Record>) -> Result<(), String> {
  let mut rows = stored.into_iter();
  while let Some(record) = rows.next() {
    let steps = commit_journal(&mut engine.world, &record);
    if !steps.is_empty() {
      journal::batch_event("follow");
    }
    match apply_follow_ups(engine, steps) {
      Ok(()) => {}
      Err((err, rest)) => {
        requeue_remaining(engine, &mut rows, rest);
        return Err(err);
      }
    }
  }
  Ok(())
}

fn apply_follow_ups(engine: &mut Engine, steps: Vec<Step>) -> Result<(), (String, Vec<Step>)> {
  let mut pending = steps.into_iter();
  while let Some(step) = pending.next() {
    if let Err(err) = engine.apply_step(step) {
      return Err((err, pending.collect()));
    }
  }
  Ok(())
}

fn requeue_remaining(
  engine: &mut Engine,
  rows: &mut impl Iterator<Item = Record>,
  mut rest: Vec<Step>,
) {
  for later in rows {
    rest.extend(commit_journal(&mut engine.world, &later));
  }
  requeue_steps(engine, rest);
}

fn requeue_steps(engine: &mut Engine, rest: Vec<Step>) {
  for step in rest.into_iter().rev() {
    engine.world.outbox.push_front(step);
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

#[cfg(test)]
#[path = "tests/outbox_flush_test.rs"]
mod tests;
