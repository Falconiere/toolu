//! `install` and `update` on Cursor's local plugins directory.

use crate::catalog::Catalog;
use crate::catalog::order::{dependents_of, install_order};
use crate::cursor::Cursor;
use crate::cursor::store::{State, version_of};
use crate::fail::{Fail, Res};
use crate::step::{Status, Step};
use crate::verbs::install::CORE;

impl Cursor {
  fn tag(&self) -> &str {
    &self.checkout.tag
  }

  /// The version the tag offers for `name`, if the tag has the plugin.
  fn offered(&self, name: &str) -> Option<String> {
    version_of(&self.source(name))
  }

  /// Copies `name` from the checkout; a plugin the tag lacks fails.
  fn copy(&self, name: &str, status: Status, argv: Vec<String>) -> Step {
    let source = self.source(name);
    if !source.is_dir() {
      let detail = format!("{} has no plugins/{name}", self.tag());
      return Step::new(name, Status::Failed, detail, argv);
    }
    match self.store.put(&source, name, self.tag()) {
      Ok(()) => {
        let version = self
          .offered(name)
          .unwrap_or_else(|| "unversioned".to_owned());
        Step::new(
          name,
          status,
          format!("{} at {version}", status.name()),
          argv,
        )
      }
      Err(detail) => Step::new(name, Status::Failed, detail, argv),
    }
  }

  fn install_one(&self, name: &str, dry_run: bool) -> Step {
    let argv = self.copy_argv(name);
    match self.store.state(name) {
      State::Foreign => Step::new(
        name,
        Status::Skew,
        format!(
          "{} is a local plugin toolu did not install; left untouched",
          self.store.path(name).display()
        ),
        argv,
      ),
      State::Ours(have) => {
        let offered = self.offered(name);
        let shown = have
          .clone()
          .unwrap_or_else(|| "an unknown version".to_owned());
        match offered {
          Some(offer) if have.as_deref() != Some(offer.as_str()) => Step::new(
            name,
            Status::Skew,
            format!(
              "installed at {shown}, {} offers {offer}; left untouched. Run `toolu plugins update {name} --host cursor` to change it.",
              self.tag()
            ),
            argv,
          ),
          _same => Step::new(
            name,
            Status::Already,
            format!("already installed at {shown}"),
            argv,
          ),
        }
      }
      State::Absent if dry_run => Step::new(name, Status::Skipped, "dry run", argv),
      State::Absent => self.copy(name, Status::Installed, argv),
    }
  }
}

/// Installs `requested` (all when empty) with their dependencies; a failed core
/// plugin skips its dependents.
pub(crate) fn install(
  cursor: &Cursor,
  catalog: &Catalog,
  requested: &[String],
  dry_run: bool,
) -> Res<Vec<Step>> {
  let order = install_order(catalog, requested)?;
  if !dry_run {
    cursor.sync()?;
  }
  let dependents = dependents_of(catalog, CORE);
  let mut core_failed = false;
  let mut steps = Vec::new();
  for name in &order {
    let step = if core_failed && dependents.contains(name) {
      Step::new(
        name,
        Status::Skipped,
        format!("skipped: {CORE} failed"),
        cursor.copy_argv(name),
      )
    } else {
      cursor.install_one(name, dry_run)
    };
    core_failed |= name == CORE && step.failed();
    steps.push(step);
  }
  Ok(steps)
}

fn update_one(cursor: &Cursor, name: &str, dry_run: bool) -> Step {
  let argv = cursor.copy_argv(name);
  let State::Ours(have) = cursor.store.state(name) else {
    let absent = cursor.store.state(name) == State::Absent;
    return if absent {
      Step::new(
        name,
        Status::Failed,
        "not installed; run `toolu plugins install` first",
        argv,
      )
    } else {
      Step::new(
        name,
        Status::Skew,
        "a local plugin toolu did not install; left untouched",
        argv,
      )
    };
  };
  match cursor.offered(name) {
    Some(want) if have.as_deref() == Some(want.as_str()) => {
      Step::new(name, Status::Current, format!("current at {want}"), argv)
    }
    _behind if dry_run => Step::new(name, Status::Skipped, "dry run", argv),
    _behind => cursor.copy(name, Status::Updated, argv),
  }
}

/// Updates `names`, or every plugin toolu installed when empty.
pub(crate) fn update(cursor: &Cursor, names: &[String], dry_run: bool) -> Res<Vec<Step>> {
  if !dry_run {
    cursor.sync()?;
  }
  let targets = if names.is_empty() {
    cursor.store.ours()
  } else {
    names.to_vec()
  };
  if targets.is_empty() {
    return Err(Fail::failed(
      "cursor has no toolu plugins installed, so there is nothing to update",
    ));
  }
  Ok(
    targets
      .iter()
      .map(|name| update_one(cursor, name, dry_run))
      .collect(),
  )
}

#[cfg(test)]
#[path = "tests/change_test.rs"]
mod tests;
