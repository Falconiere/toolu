//! `install` on `OpenCode`: ensure the package entry where the host loads it,
//! then the requested selection (the former TypeScript installer's `src/opencode/install.ts`).

use std::path::Path;

use crate::catalog::Catalog;
use crate::catalog::order::install_order;
use crate::fail::{Fail, Res};
use crate::opencode::entries::{
  PACKAGE, PluginEntry, effective_entry, governing_global, plugin_array, plugin_len, toolu_entries,
};
use crate::opencode::jsonc::{ConfigFile, add_plugin_key, new_config_text, parse_config};
use crate::opencode::paths::Scope;
use crate::opencode::selection::{closure, selection_text};
use crate::opencode::splice;
use crate::opencode::state::{Plan, State, Write};
use crate::step::{Status, Step};

/// What to install.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Install<'a> {
  /// The plugins named; none means every plugin.
  pub(crate) names: &'a [String],
  /// `--scope`, mapped.
  pub(crate) scope: Option<Scope>,
  /// The spec to write.
  pub(crate) target: String,
}

fn nth(files: &[ConfigFile], index: usize) -> Res<&ConfigFile> {
  files
    .get(index)
    .ok_or_else(|| Fail::failed(format!("missing config candidate {index}")))
}

/// The file a new entry goes to, so the host loads it rather than shadowing it.
fn entry_file(state: &State, scope: Scope) -> Res<&ConfigFile> {
  let files = state.files(scope);
  if scope == Scope::Global {
    if let Some(file) = governing_global(files)? {
      return Ok(file);
    }
    return match files.iter().rev().find(|file| file.text.is_some()) {
      Some(file) => Ok(file),
      None => nth(files, 1),
    };
  }
  for file in files.iter().rev() {
    if plugin_array(file)?.is_some() {
      return Ok(file);
    }
  }
  let jsonc = nth(files, 1)?;
  if jsonc.text.is_none() {
    nth(files, 0)
  } else {
    Ok(jsonc)
  }
}

fn with_entry(file: &ConfigFile, spec: &str) -> Res<String> {
  let Some(text) = &file.text else {
    return Ok(new_config_text(spec));
  };
  Ok(match plugin_array(file)? {
    None => add_plugin_key(text, spec),
    Some(_array) => splice::append(text, "plugin", spec),
  })
}

fn present_step(entry: &PluginEntry, target: &str) -> Step {
  let place = format!("{} in {}", entry.spec, entry.file);
  if entry.spec == target {
    return Step::new(
      PACKAGE,
      Status::Already,
      format!("already configured: {place}"),
      Vec::new(),
    );
  }
  Step::new(
    PACKAGE,
    Status::Skew,
    format!(
      "configured as {place}; this CLI installs {target}; left untouched. Run `toolu plugins update --host opencode` to change it."
    ),
    Vec::new(),
  )
}

/// The state as it would read after `write`.
fn after(state: &State, write: &Write) -> Res<State> {
  let mut next = state.clone();
  for file in next.global.iter_mut().chain(next.project.iter_mut()) {
    if file.path == write.path {
      file.data = Some(parse_config(&file.path, &write.text)?);
      file.text = Some(write.text.clone());
    }
  }
  Ok(next)
}

/// A note when a project file's empty `plugin` list keeps the host from loading toolu.
fn shadow_note(state: &State) -> Res<String> {
  if effective_entry(&state.global, &state.project)?.is_some() {
    return Ok(String::new());
  }
  for file in state.project.iter().rev() {
    if plugin_len(file)? == Some(0) {
      return Ok(format!(
        "; no effect here: {} sets an empty plugin list",
        file.path.display()
      ));
    }
  }
  Ok(String::new())
}

/// A note when the entry fills a project `plugin: []`, which then stops
/// resetting the earlier layers.
fn lift_note(state: &State, file: &ConfigFile) -> Res<String> {
  let Some(at) = state
    .project
    .iter()
    .position(|candidate| candidate.path == file.path)
  else {
    return Ok(String::new());
  };
  if plugin_len(file)? != Some(0) {
    return Ok(String::new());
  }
  let mut earlier: Vec<&ConfigFile> = governing_global(&state.global)?.into_iter().collect();
  earlier.extend(state.project.iter().take(at));
  for candidate in earlier {
    if plugin_len(candidate)?.unwrap_or(0) > 0 {
      return Ok(format!(
        "; {} no longer empties the plugin list, so plugins from earlier config files load here again",
        file.path.display()
      ));
    }
  }
  Ok(String::new())
}

/// Entries the host can load from `scope`; a lower-priority global array never loads.
fn loadable(state: &State, scope: Scope) -> Res<Vec<PluginEntry>> {
  match scope {
    Scope::Project => state.entries_in(Scope::Project),
    Scope::Global => match governing_global(&state.global)? {
      Some(file) => toolu_entries(file),
      None => Ok(Vec::new()),
    },
  }
}

/// With no scope, the entry the host loads, or any loadable one when a reset shadows it.
pub(crate) fn existing_entry(state: &State, scope: Option<Scope>) -> Res<Option<PluginEntry>> {
  if let Some(scope) = scope {
    return Ok(loadable(state, scope)?.into_iter().next());
  }
  if let Some(entry) = effective_entry(&state.global, &state.project)? {
    return Ok(Some(entry));
  }
  let project = loadable(state, Scope::Project)?.into_iter().next();
  Ok(project.or(loadable(state, Scope::Global)?.into_iter().next()))
}

fn package_step(state: &State, request: &Install<'_>) -> Res<(Step, Option<Write>)> {
  if let Some(present) = existing_entry(state, request.scope)? {
    let mut step = present_step(&present, &request.target);
    let note = shadow_note(state)?;
    if !note.is_empty() {
      step.status = Status::Skew;
      step.detail.push_str(&note);
    }
    return Ok((step, None));
  }
  let file = entry_file(state, request.scope.unwrap_or(Scope::Global))?;
  let write = Write {
    path: file.path.clone(),
    text: with_entry(file, &request.target)?,
  };
  let note = shadow_note(&after(state, &write)?)?;
  let shown = file.path.display();
  let status = if note.is_empty() {
    Status::Installed
  } else {
    Status::Skew
  };
  let detail = format!(
    "added {} to {shown}{note}{}",
    request.target,
    lift_note(state, file)?
  );
  let step = Step::new(PACKAGE, status, detail, Vec::new())
    .planned(format!("add {} to {shown}", request.target));
  Ok((step, Some(write)))
}

fn plugin_step(
  name: &str,
  added: bool,
  path: Option<&Path>,
  state: &State,
  overridden: &str,
) -> Step {
  let skill_off = if state.disabled.iter().any(|off| off == name) {
    format!(
      " (skills.{name} is false in {}, so it stays off)",
      state.paths.toolu_config.display()
    )
  } else {
    String::new()
  };
  let off = format!("{skill_off}{overridden}");
  match (added, path) {
    (false, _any) => Step::new(
      name,
      Status::Already,
      format!("already enabled{off}"),
      Vec::new(),
    ),
    (true, None) => Step::new(
      name,
      Status::Installed,
      format!("enabled (no selection file, so every plugin is on){off}"),
      Vec::new(),
    ),
    (true, Some(path)) => Step::new(
      name,
      Status::Installed,
      format!("enabled in {}{off}", path.display()),
      Vec::new(),
    )
    .planned(format!("enable {name} in {}", path.display())),
  }
}

/// The names of `want` that `base` (closed over dependencies) lacks, and
/// `base` with them appended.
fn grow(catalog: &Catalog, base: Vec<String>, want: &[String]) -> (Vec<String>, Vec<String>) {
  let have = closure(catalog, &base);
  let added: Vec<String> = want
    .iter()
    .filter(|name| !have.contains(name))
    .cloned()
    .collect();
  let mut next = base;
  for name in &added {
    if !next.contains(name) {
      next.push(name.clone());
    }
  }
  (added, next)
}

/// The selection changes of an install.
fn selection_plan(
  state: &State,
  catalog: &Catalog,
  request: &Install<'_>,
  installed_before: bool,
) -> Res<Plan> {
  let scope = request
    .scope
    .unwrap_or_else(|| state.default_selection_scope());
  let all = catalog.names();
  let want = if request.names.is_empty() {
    all.clone()
  } else {
    install_order(catalog, request.names)?
  };
  let base: Vec<String> = match state.governing_selection(scope) {
    Some(listed) => listed.to_vec(),
    None if installed_before => all.clone(),
    None => Vec::new(),
  };
  let (added, next) = grow(catalog, base, &want);
  let no_files =
    state.selection(Scope::Global).is_none() && state.selection(Scope::Project).is_none();
  let path = state.paths.selection(scope);
  let skip = added.is_empty() || (no_files && all.iter().all(|name| next.contains(name)));
  let writes = if skip {
    Vec::new()
  } else {
    vec![Write {
      path: path.to_path_buf(),
      text: selection_text(&next),
    }]
  };
  let overridden = state.overridden_note(scope);
  let shown = if skip { None } else { Some(path) };
  let steps = want
    .iter()
    .map(|name| plugin_step(name, added.contains(name), shown, state, &overridden))
    .collect();
  Ok(Plan { steps, writes })
}

/// The package entry and the requested selection.
pub(crate) fn plan(state: &State, catalog: &Catalog, request: &Install<'_>) -> Res<Plan> {
  let installed_before = existing_entry(state, None)?.is_some();
  let (step, write) = package_step(state, request)?;
  let selection = selection_plan(state, catalog, request, installed_before)?;
  let mut steps = vec![step];
  steps.extend(selection.steps);
  let mut writes: Vec<Write> = write.into_iter().collect();
  writes.extend(selection.writes);
  Ok(Plan { steps, writes })
}

#[cfg(test)]
#[path = "tests/install_test.rs"]
mod tests;
