//! `install` on Claude Code and Codex: add the marketplace, then each plugin in
//! dependency order. A failure of the core plugin skips its dependents, which
//! cannot work without it; any other failure is recorded and the rest continue.

use crate::args::Scope;
use crate::catalog::Catalog;
use crate::catalog::order::{dependents_of, install_order};
use crate::fail::Res;
use crate::host::adapter::Installed;
use crate::host::run::first_line;
use crate::step::{Status, Step};
use crate::verbs::{Driver, find};

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
  // A failed marketplace add is not fatal (it may be there already), but a
  // plugin that then fails to install says so.
  let added = if dry_run {
    None
  } else {
    Some(driver.run(&adapter.add_marketplace()))
  };
  let add_note = added
    .filter(|ran| !ran.ok())
    .map(|ran| {
      format!(
        "; marketplace add failed: {}",
        first_line(&ran.stderr, "no output")
      )
    })
    .unwrap_or_default();
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
      steps.push(present(name, have, find(&offered, name), argv));
      continue;
    }
    if dry_run {
      steps.push(Step::new(name, Status::Skipped, "dry run", argv));
      continue;
    }
    let ran = driver.run(&argv);
    if ran.ok() {
      steps.push(Step::new(name, Status::Installed, "installed", argv));
    } else {
      core_failed |= name == CORE;
      let detail = format!("{}{add_note}", first_line(&ran.stderr, "install failed"));
      steps.push(Step::new(name, Status::Failed, detail, argv));
    }
  }
  Ok(steps)
}

#[cfg(test)]
#[path = "tests/install_test.rs"]
mod tests;
