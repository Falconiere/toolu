//! Carrying out an `OpenCode` plan: a dry run writes nothing; otherwise every
//! planned file is read again first, and if any changed since the run read it
//! (another `toolu plugins`, an editor saving), nothing is written.

use std::path::Path;

use crate::fail::{Fail, Res};
use crate::opencode::state::{Plan, State};
use crate::opencode::write::write_atomic;
use crate::step::Step;

fn current(path: &Path) -> Res<Option<String>> {
  match std::fs::read_to_string(path) {
    Ok(text) => Ok(Some(text)),
    Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
    Err(err) => Err(Fail::failed(format!(
      "cannot read {}: {err}",
      path.display()
    ))),
  }
}

/// Writes `plan` unless `dry_run`, and returns its steps.
pub(crate) fn apply(state: &State, plan: Plan, dry_run: bool) -> Res<Vec<Step>> {
  if dry_run {
    return Ok(plan.steps);
  }
  for write in &plan.writes {
    let Some(original) = state.original(&write.path) else {
      continue;
    };
    if current(&write.path)?.as_ref() != original.as_ref() {
      return Err(Fail::failed(format!(
        "{} changed while toolu ran; nothing was written",
        write.path.display()
      )));
    }
  }
  for write in &plan.writes {
    write_atomic(&write.path, &write.text)
      .map_err(|err| Fail::failed(format!("cannot write {}: {err}", write.path.display())))?;
  }
  Ok(plan.steps)
}

#[cfg(test)]
#[path = "tests/apply_test.rs"]
mod tests;
