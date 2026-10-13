//! `remove` on Claude Code and Codex: each name uninstalled independently.

use crate::host::run::first_line;
use crate::step::{Status, Step};
use crate::verbs::Driver;

/// Uninstalls each of `names`, reporting each; a dry run runs nothing.
pub(crate) fn run(driver: &Driver, names: &[String], dry_run: bool) -> Vec<Step> {
  names
    .iter()
    .map(|name| {
      let argv = driver.adapter.remove(name);
      if dry_run {
        return Step::new(name, Status::Skipped, "dry run", argv);
      }
      let ran = driver.run(&argv);
      if ran.ok() {
        Step::new(name, Status::Removed, "removed", argv)
      } else {
        Step::new(
          name,
          Status::Failed,
          first_line(&ran.stderr, "remove failed"),
          argv,
        )
      }
    })
    .collect()
}

#[cfg(test)]
#[path = "tests/remove_test.rs"]
mod tests;
