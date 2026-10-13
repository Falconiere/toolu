//! `remove` on `OpenCode`: `remove toolu` drops the package entry; other names
//! leave the selection, unless an enabled plugin still needs them
//! (`tools/toolu-cli` `src/opencode/remove.ts`).

use crate::catalog::Catalog;
use crate::catalog::order::install_order;
use crate::fail::{Fail, Res};
use crate::opencode::entries::{PACKAGE, plugin_len, toolu_entries};
use crate::opencode::jsonc::{ConfigFile, remove_key};
use crate::opencode::paths::Scope;
use crate::opencode::selection::{closure, dependents_blocking, selection_text};
use crate::opencode::splice;
use crate::opencode::state::{Plan, State, Write};
use crate::step::{Status, Step};
use crate::verbs::install::CORE;

fn done(name: &str, detail: impl Into<String>) -> Step {
  Step::new(name, Status::Removed, detail, Vec::new())
}

/// The scopes a package-level verb acts on; ambiguity is a usage error, not a guess.
pub(crate) fn package_scopes(state: &State, scope: Option<Scope>) -> Res<Vec<Scope>> {
  if let Some(scope) = scope {
    return Ok(vec![scope]);
  }
  let scopes = state.installed_scopes()?;
  if scopes.len() > 1 {
    return Err(Fail::usage(format!(
      "{PACKAGE} is configured in both global and project OpenCode config; choose one with --scope user or --scope project"
    )));
  }
  Ok(scopes)
}

/// The files of `scopes` holding a toolu entry.
pub(crate) fn files_with_entries<'a>(
  state: &'a State,
  scopes: &[Scope],
) -> Res<Vec<&'a ConfigFile>> {
  let mut files = Vec::new();
  for scope in scopes {
    for file in state.files(*scope) {
      if !toolu_entries(file)?.is_empty() {
        files.push(file);
      }
    }
  }
  Ok(files)
}

/// Drops every toolu entry and keeps the others; an array left empty loses its
/// key, since `[]` resets the host's list.
fn without_entries(file: &ConfigFile) -> Res<String> {
  let text = file.text.clone().unwrap_or_default();
  let entries = toolu_entries(file)?;
  if Some(entries.len()) == plugin_len(file)? {
    return Ok(remove_key(&text, "plugin"));
  }
  Ok(entries.iter().rev().fold(text, |current, entry| {
    splice::remove(&current, "plugin", entry.index)
  }))
}

/// A lower global file whose list applies again once `file` loses its key.
fn resurfaced(state: &State, file: &ConfigFile) -> Res<String> {
  let Some(index) = state
    .global
    .iter()
    .position(|candidate| candidate.path == file.path)
  else {
    return Ok(String::new());
  };
  if index == 0 || Some(toolu_entries(file)?.len()) != plugin_len(file)? {
    return Ok(String::new());
  }
  for lower in state.global.iter().take(index).rev() {
    if plugin_len(lower)?.is_some() {
      return Ok(format!(
        "; {}'s plugin list applies again",
        lower.path.display()
      ));
    }
  }
  Ok(String::new())
}

fn remove_package(state: &State, names: &[String], scope: Option<Scope>) -> Res<Plan> {
  let files = files_with_entries(state, &package_scopes(state, scope)?)?;
  let others = names
    .iter()
    .filter(|name| *name != CORE)
    .map(|name| done(name, format!("removed with {PACKAGE}")));
  if files.is_empty() {
    let mut steps = vec![done(
      CORE,
      format!("{PACKAGE} is not configured; nothing to remove"),
    )];
    steps.extend(others);
    return Ok(Plan {
      steps,
      writes: Vec::new(),
    });
  }
  let kept = [Scope::Project, Scope::Global]
    .into_iter()
    .filter(|scope| state.selection(*scope).is_some())
    .map(|scope| {
      format!(
        "; selection kept at {}",
        state.paths.selection(scope).display()
      )
    })
    .collect::<Vec<_>>()
    .concat();
  let place = files
    .iter()
    .map(|file| file.path.display().to_string())
    .collect::<Vec<_>>()
    .join(", ");
  let mut notes = String::new();
  let mut writes = Vec::new();
  for file in &files {
    notes.push_str(&resurfaced(state, file)?);
    writes.push(Write {
      path: file.path.clone(),
      text: without_entries(file)?,
    });
  }
  let step = done(CORE, format!("removed {PACKAGE} from {place}{kept}{notes}"))
    .planned(format!("remove {PACKAGE} from {place}"));
  let mut steps = vec![step];
  steps.extend(others);
  Ok(Plan { steps, writes })
}

/// Names that can go: enabled, and not needed by an enabled plugin that stays.
fn removable(catalog: &Catalog, enabled: &[String], names: &[String]) -> Vec<String> {
  let mut removing: Vec<String> = names
    .iter()
    .filter(|name| enabled.contains(name))
    .cloned()
    .collect();
  loop {
    let kept: Vec<String> = removing
      .iter()
      .filter(|name| dependents_blocking(catalog, enabled, &removing, name).is_empty())
      .cloned()
      .collect();
    if kept.len() == removing.len() {
      return removing;
    }
    removing = kept;
  }
}

/// What a leaf removal decided, to report each name.
struct Leaves<'a> {
  catalog: &'a Catalog,
  enabled: &'a [String],
  removing: &'a [String],
  path: String,
  overridden: String,
}

impl Leaves<'_> {
  /// The step of `name`: absent, disabled, or refused while another needs it.
  fn step(&self, name: &str) -> Step {
    let (path, overridden) = (&self.path, &self.overridden);
    if !self.enabled.iter().any(|enabled| enabled == name) {
      return done(name, format!("not enabled; nothing to change{overridden}"));
    }
    if self.removing.iter().any(|removing| removing == name) {
      return done(name, format!("disabled in {path}{overridden}"))
        .planned(format!("disable {name} in {path}"));
    }
    let blockers = dependents_blocking(self.catalog, self.enabled, self.removing, name);
    let detail = format!("required by {}; remove them too", blockers.join(", "));
    Step::new(name, Status::Failed, detail, Vec::new())
  }
}

fn remove_leaves(
  state: &State,
  catalog: &Catalog,
  names: &[String],
  scope: Option<Scope>,
) -> Res<Plan> {
  if state.installed_scopes()?.is_empty() {
    let steps = names.iter().map(|name| {
      done(
        name,
        format!("{PACKAGE} is not configured; nothing to remove"),
      )
    });
    return Ok(Plan {
      steps: steps.collect(),
      writes: Vec::new(),
    });
  }
  let target = scope.unwrap_or_else(|| state.default_selection_scope());
  let base = state
    .governing_selection(target)
    .map_or_else(|| catalog.names(), <[String]>::to_vec);
  let enabled = closure(catalog, &base);
  let removing = removable(catalog, &enabled, names);
  let leaves = Leaves {
    catalog,
    enabled: &enabled,
    removing: &removing,
    path: state.paths.selection(target).display().to_string(),
    overridden: state.overridden_note(target),
  };
  let steps = names.iter().map(|name| leaves.step(name)).collect();
  let next: Vec<String> = base
    .into_iter()
    .filter(|name| !removing.contains(name))
    .collect();
  let writes = if removing.is_empty() {
    Vec::new()
  } else {
    vec![Write {
      path: state.paths.selection(target).to_path_buf(),
      text: selection_text(&next),
    }]
  };
  Ok(Plan { steps, writes })
}

/// `remove toolu` drops the package; other names leave the selection.
pub(crate) fn plan(
  state: &State,
  catalog: &Catalog,
  names: &[String],
  scope: Option<Scope>,
) -> Res<Plan> {
  install_order(catalog, names)?;
  if names.iter().any(|name| name == CORE) {
    return remove_package(state, names, scope);
  }
  remove_leaves(state, catalog, names, scope)
}

#[cfg(test)]
#[path = "tests/remove_test.rs"]
mod tests;
