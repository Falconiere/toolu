//! `install`: make the marketplace ready, then each plugin in dependency order,
//! then settle hook approval (Muse). A failure of the core plugin skips its
//! dependents, which cannot work without it; any other failure is recorded and
//! the rest continue.

use crate::args::{Scope, Verb};
use crate::catalog::Catalog;
use crate::catalog::order::{dependents_of, install_order};
use crate::fail::Res;
use crate::host::adapter::Installed;
use crate::host::run::first_line;
use crate::step::{Status, Step};
use crate::verbs::{Driver, find, foreign_step, prepare, trust};

/// The plugin every other one builds on.
pub(crate) const CORE: &str = "toolu";

fn present(name: &str, have: &Installed, offered: Option<&Installed>, argv: Vec<String>) -> Step {
  match offered {
    Some(offer) if offer.version != have.version => Step::new(
      name,
      Status::Skew,
      format!(
        "installed at {}, marketplace offers {}; left untouched. Run `toolu plugins update {name}` to change it.",
        have.version, offer.version
      ),
      argv,
    ),
    _same => Step::new(
      name,
      Status::Already,
      format!("already installed at {}", have.version),
      argv,
    ),
  }
}

/// Runs the install of `name`; a failure names the marketplace hint too.
fn attempt(driver: &Driver, name: &str, argv: Vec<String>, add_note: &str) -> Step {
  let ran = driver.run(&argv);
  if ran.ok() {
    return Step::new(name, Status::Installed, "installed", argv);
  }
  let detail = format!("{}{add_note}", first_line(&ran.stderr, "install failed"));
  Step::new(name, Status::Failed, detail, argv)
}

/// Installs `requested` (all when empty) with their dependencies.
pub(crate) fn run(
  driver: &Driver,
  catalog: &Catalog,
  requested: &[String],
  scope: Option<Scope>,
  dry_run: bool,
) -> Res<Vec<Step>> {
  let order = install_order(catalog, requested)?;
  let adapter = driver.adapter;
  let add_note = if dry_run {
    String::new()
  } else {
    prepare::run(driver, Verb::Install)?
  };
  let installed = driver.versions(&adapter.list_installed());
  let offered = driver.versions(&adapter.list_available());
  let dependents = dependents_of(catalog, CORE);
  let mut core_failed = false;
  let mut steps = Vec::new();
  for name in &order {
    let argv = adapter.install(name, scope);
    if core_failed && dependents.contains(name) {
      steps.push(Step::new(
        name,
        Status::Skipped,
        format!("skipped: {CORE} failed"),
        argv,
      ));
      continue;
    }
    if let Some(have) = find(&installed, name) {
      steps.push(if driver.foreign(have) {
        foreign_step(name, have, argv)
      } else {
        present(name, have, find(&offered, name), argv)
      });
      continue;
    }
    if dry_run {
      steps.push(Step::new(name, Status::Skipped, "dry run", argv));
      continue;
    }
    let step = attempt(driver, name, argv, &add_note);
    core_failed |= name == CORE && step.failed();
    steps.push(step);
  }
  if dry_run {
    return Ok(steps);
  }
  Ok(trust::settle(driver, steps))
}

#[cfg(test)]
#[path = "tests/install_test.rs"]
mod tests;
