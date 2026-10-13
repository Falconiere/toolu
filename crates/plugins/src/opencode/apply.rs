//! Carrying out an `OpenCode` plan. A dry run writes nothing. Otherwise every
//! planned file is read again first, and if any changed since the run read it
//! (another `toolu plugins`, an editor saving), nothing is written; an edited
//! config must still be valid JSONC and must differ from what was read. Every
//! file is staged beside its target before any is renamed onto it.

use std::path::Path;

use crate::fail::{Fail, Res};
use crate::opencode::jsonc::parse_config;
use crate::opencode::state::{Plan, State, Write};
use crate::opencode::write::{Staged, stage};
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

/// Refuses a write whose file changed since the read, or a config edit that
/// is no longer JSONC or changed nothing.
fn check(state: &State, write: &Write) -> Res<()> {
  let shown = write.path.display();
  let Some(original) = state.original(&write.path) else {
    return Ok(());
  };
  if current(&write.path)?.as_ref() != original.as_ref() {
    return Err(Fail::failed(format!(
      "{shown} changed while toolu ran; nothing was written"
    )));
  }
  if state.is_config(&write.path) {
    parse_config(&write.path, &write.text)?;
    if original.as_deref() == Some(write.text.as_str()) {
      return Err(Fail::failed(format!(
        "the edit of {shown} changed nothing; nothing was written"
      )));
    }
  }
  Ok(())
}

/// Writes `plan` unless `dry_run`, and returns its steps.
pub(crate) fn apply(state: &State, plan: Plan, dry_run: bool) -> Res<Vec<Step>> {
  if dry_run {
    return Ok(plan.steps);
  }
  for write in &plan.writes {
    check(state, write)?;
  }
  let mut staged: Vec<Staged> = Vec::new();
  for write in &plan.writes {
    match stage(&write.path, &write.text) {
      Ok(file) => staged.push(file),
      Err(err) => {
        staged.iter().for_each(Staged::discard);
        return Err(Fail::failed(format!(
          "cannot write {}: {err}; nothing was written",
          write.path.display()
        )));
      }
    }
  }
  let mut written: Vec<String> = Vec::new();
  for (file, write) in staged.into_iter().zip(&plan.writes) {
    if let Err(err) = file.commit() {
      let done = if written.is_empty() {
        "nothing".to_owned()
      } else {
        written.join(", ")
      };
      return Err(Fail::failed(format!(
        "cannot write {}: {err}; already written: {done}",
        write.path.display()
      )));
    }
    written.push(write.path.display().to_string());
  }
  Ok(plan.steps)
}

#[cfg(test)]
#[path = "tests/apply_test.rs"]
mod tests;
