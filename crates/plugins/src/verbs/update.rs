//! `update`: only what differs from the offered version, then hook approval
//! (Muse). On Muse the checkout moves to this binary's tag first, and a plugin
//! installed from elsewhere is left alone. On Codex the marketplace is pinned
//! to the tag first; a re-pin adds every installed plugin again (#626). Claude
//! Code offers no versions, so each plugin is updated and its result line read.
//! A plugin the catalog no longer has is retired: never updated, only named.

use crate::args::Verb;
use crate::catalog::Catalog;
use crate::fail::{Fail, Res};
use crate::host::adapter::{Adapter, Installed};
use crate::host::claude;
use crate::host::run::first_line;
use crate::step::{Status, Step};
use crate::verbs::{Driver, find, foreign_step, pin, prepare, retired_step, trust};

/// Updates `names`, or every installed toolu plugin when empty; a dry run runs
/// no update.
pub(crate) fn run(
  driver: &Driver,
  catalog: &Catalog,
  names: &[String],
  dry_run: bool,
) -> Res<Vec<Step>> {
  let adapter = driver.adapter;
  driver.begin(0, "preparing the marketplace");
  let repinned = !dry_run && prepare::run(driver, Verb::Update)?.repinned;
  let installed = driver.versions(&adapter.list_installed());
  let offered = adapter
    .list_available()
    .map(|argv| driver.versions(&argv))
    .unwrap_or_default();
  let targets = targets(driver, names, &installed);
  if targets.is_empty() {
    // An empty request and an empty listing mean the host said nothing; saying
    // so beats an empty report that reads as success.
    return Err(Fail::failed(format!(
      "{} reported no installed plugins, so there is nothing to update",
      adapter.bin()
    )));
  }
  driver.begin(targets.len(), "reading the host");
  let steps = targets
    .iter()
    .map(|name| {
      driver.working(&format!("updating {name}"));
      let foreign = find(&installed, name).is_some_and(|entry| driver.foreign(entry));
      let step = if !foreign && catalog.entry(name).is_none() {
        retired_step(name, adapter.bin(), adapter.update(name))
      } else if repinned && !foreign {
        pin::refresh(driver, name)
      } else {
        one(driver, name, &installed, &offered, dry_run)
      };
      driver.advance();
      step
    })
    .collect();
  let steps = if dry_run {
    steps
  } else {
    trust::settle(driver, steps)
  };
  driver.end();
  Ok(steps)
}

/// `names`, or each installed plugin toolu's marketplace put there, once.
fn targets(driver: &Driver, names: &[String], installed: &[Installed]) -> Vec<String> {
  if !names.is_empty() {
    return names.to_vec();
  }
  let mut targets: Vec<String> = Vec::new();
  for entry in installed.iter().filter(|entry| !driver.foreign(entry)) {
    if !targets.contains(&entry.name) {
      targets.push(entry.name.clone());
    }
  }
  targets
}

/// Updates `name` unless it is current or installed from elsewhere.
fn one(
  driver: &Driver,
  name: &str,
  installed: &[Installed],
  offered: &[Installed],
  dry_run: bool,
) -> Step {
  let argv = driver.adapter.update(name);
  let entry = find(installed, name);
  if let Some(entry) = entry
    && driver.foreign(entry)
  {
    return foreign_step(name, entry, argv);
  }
  let have = entry.map(|entry| entry.version.as_str());
  let want = find(offered, name).map(|entry| entry.version.as_str());
  if let (Some(have), Some(want)) = (have, want)
    && have == want
  {
    return Step::new(name, Status::Current, format!("current at {have}"), argv);
  }
  if dry_run {
    return Step::new(name, Status::Skipped, "dry run", argv);
  }
  let ran = driver.run(&argv);
  if driver.adapter == Adapter::Claude {
    return claude::update_step(name, &ran, argv);
  }
  if !ran.ok() {
    return Step::new(
      name,
      Status::Failed,
      first_line(&ran.stderr, "update failed"),
      argv,
    );
  }
  let detail = want.map_or_else(|| "updated".to_owned(), |want| format!("updated to {want}"));
  Step::new(name, Status::Updated, detail, argv)
}

#[cfg(test)]
#[path = "tests/update_test.rs"]
mod tests;
