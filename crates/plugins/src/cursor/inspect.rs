//! `list` and `remove` on Cursor's local plugins directory.

use crate::catalog::Catalog;
use crate::cursor::Cursor;
use crate::cursor::store::State;
use crate::report::Row;
use crate::step::{Status, Step};

/// Every catalog plugin with what toolu installed in Cursor.
pub(crate) fn list(cursor: &Cursor, catalog: &Catalog) -> Vec<Row> {
  catalog
    .names()
    .into_iter()
    .map(|name| {
      let state = cursor.store.state(&name);
      let version = match &state {
        State::Ours(version) => version.clone(),
        State::Absent | State::Foreign => None,
      };
      Row {
        installed: matches!(state, State::Ours(_)),
        enabled: matches!(state, State::Ours(_)),
        version,
        name,
      }
    })
    .collect()
}

fn remove_one(cursor: &Cursor, name: &str, dry_run: bool) -> Step {
  let argv = vec![
    "rm".to_owned(),
    "-r".to_owned(),
    cursor.store.path(name).display().to_string(),
  ];
  match cursor.store.state(name) {
    State::Foreign => Step::new(
      name,
      Status::Skew,
      "a local plugin toolu did not install; left untouched",
      argv,
    ),
    State::Absent => Step::new(name, Status::Removed, "not installed", argv),
    State::Ours(_) if dry_run => Step::new(name, Status::Skipped, "dry run", argv),
    State::Ours(_) => match cursor.store.take(name) {
      Ok(()) => Step::new(name, Status::Removed, "removed", argv),
      Err(detail) => Step::new(name, Status::Failed, detail, argv),
    },
  }
}

/// Removes each of `names` that toolu installed.
pub(crate) fn remove(cursor: &Cursor, names: &[String], dry_run: bool) -> Vec<Step> {
  names
    .iter()
    .map(|name| remove_one(cursor, name, dry_run))
    .collect()
}

#[cfg(test)]
#[path = "tests/inspect_test.rs"]
mod tests;
