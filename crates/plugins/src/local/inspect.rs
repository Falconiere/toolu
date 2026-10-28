//! `list` and `remove` on a local-directory host's plugins directory.

use crate::catalog::Catalog;
use crate::local::Local;
use crate::local::store::State;
use crate::report::Row;
use crate::step::{Status, Step};

/// Every catalog plugin with what toolu installed on the host.
pub(crate) fn list(local: &Local, catalog: &Catalog) -> Vec<Row> {
  catalog
    .names()
    .into_iter()
    .map(|name| {
      let state = local.store.state(&name);
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

fn remove_one(local: &Local, name: &str, dry_run: bool) -> Step {
  let argv = vec![
    "rm".to_owned(),
    "-r".to_owned(),
    local.store.path(name).display().to_string(),
  ];
  match local.store.state(name) {
    State::Foreign => Step::new(
      name,
      Status::Skew,
      "a local plugin toolu did not install; left untouched",
      argv,
    ),
    State::Absent => Step::new(name, Status::Removed, "not installed", argv),
    State::Ours(_) if dry_run => Step::new(name, Status::Skipped, "dry run", argv),
    State::Ours(_) => match local.store.take(name) {
      Ok(()) => Step::new(name, Status::Removed, "removed", argv),
      Err(detail) => Step::new(name, Status::Failed, detail, argv),
    },
  }
}

/// Removes each of `names` that toolu installed.
pub(crate) fn remove(local: &Local, names: &[String], dry_run: bool) -> Vec<Step> {
  names
    .iter()
    .map(|name| remove_one(local, name, dry_run))
    .collect()
}

#[cfg(test)]
#[path = "tests/inspect_test.rs"]
mod tests;
