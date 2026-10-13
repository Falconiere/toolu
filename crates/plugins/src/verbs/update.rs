//! `update` on Claude Code and Codex: only what differs from the offered version.

use crate::fail::{Fail, Res};
use crate::host::run::first_line;
use crate::step::{Status, Step};
use crate::verbs::{Driver, find};

/// Updates `names`, or every installed plugin when empty; a dry run runs no update.
pub(crate) fn run(driver: &Driver, names: &[String], dry_run: bool) -> Res<Vec<Step>> {
  let adapter = driver.adapter;
  let installed = driver.versions(&adapter.list_installed());
  let offered = driver.versions(&adapter.list_available());
  let targets: Vec<String> = if names.is_empty() {
    installed.iter().map(|entry| entry.name.clone()).collect()
  } else {
    names.to_vec()
  };
  if targets.is_empty() {
    // An empty request and an empty listing mean the host said nothing; saying
    // so beats an empty report that reads as success.
    return Err(Fail::failed(format!(
      "{} reported no installed plugins, so there is nothing to update",
      adapter.bin()
    )));
  }
  let steps = targets
    .iter()
    .map(|name| {
      let argv = adapter.update(name);
      let have = find(&installed, name).map(|entry| entry.version.as_str());
      let want = find(&offered, name).map(|entry| entry.version.as_str());
      if let (Some(have), Some(want)) = (have, want)
        && have == want
      {
        return Step::new(name, Status::Current, format!("current at {have}"), argv);
      }
      if dry_run {
        return Step::new(name, Status::Skipped, "dry run", argv);
      }
      let ran = driver.run(&argv);
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
    })
    .collect();
  Ok(steps)
}

#[cfg(test)]
#[path = "tests/update_test.rs"]
mod tests;
