//! `list` on Claude Code and Codex: the catalog joined with `plugin list --json`.

use crate::catalog::Catalog;
use crate::fail::{Fail, Res};
use crate::report::Row;
use crate::verbs::Driver;

/// Every catalog plugin with the host's state; a malformed listing fails.
pub(crate) fn run(driver: &Driver, catalog: &Catalog) -> Res<Vec<Row>> {
  let ran = driver.run(&driver.adapter.list_installed());
  let present = if ran.ok() {
    driver.adapter.parse_list(&ran.stdout).map_err(Fail::failed)?
  } else {
    Vec::new()
  };
  Ok(
    catalog
      .names()
      .into_iter()
      .map(|name| {
        let entry = crate::verbs::find(&present, &name);
        Row {
          installed: entry.is_some(),
          version: entry.map(|entry| entry.version.clone()),
          enabled: entry.is_some_and(|entry| entry.enabled),
          name,
        }
      })
      .collect(),
  )
}

#[cfg(test)]
#[path = "tests/list_test.rs"]
mod tests;
