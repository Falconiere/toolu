//! `list` on `OpenCode`: the catalog as the host would load it now, and a
//! one-line summary of where the package and the selection come from.

use crate::catalog::Catalog;
use crate::fail::Res;
use crate::opencode::entries::{PACKAGE, effective_entry, version_of};
use crate::opencode::selection::effective_enabled;
use crate::opencode::state::State;
use crate::report::Row;

/// The header line and one row per catalog plugin.
pub(crate) fn plan(state: &State, catalog: &Catalog) -> Res<(String, Vec<Row>)> {
  let Some(entry) = effective_entry(&state.global, &state.project)? else {
    let rows = catalog
      .names()
      .into_iter()
      .map(|name| Row {
        name,
        installed: false,
        version: None,
        enabled: false,
      })
      .collect();
    return Ok((format!("{PACKAGE}: not configured\n"), rows));
  };
  let source = state.default_selection_scope();
  let listed = state.selection(source);
  let enabled = effective_enabled(catalog, listed, &state.disabled);
  let selection = match listed {
    None => "none (every plugin)".to_owned(),
    Some(_) => state.paths.selection(source).display().to_string(),
  };
  let version = version_of(&entry.spec).to_owned();
  let rows = catalog
    .names()
    .into_iter()
    .map(|name| Row {
      enabled: enabled.contains(&name),
      name,
      installed: true,
      version: Some(version.clone()),
    })
    .collect();
  let header = format!(
    "{PACKAGE}: {} in {}; selection: {selection}\n",
    entry.spec, entry.file
  );
  Ok((header, rows))
}

#[cfg(test)]
#[path = "tests/list_test.rs"]
mod tests;
