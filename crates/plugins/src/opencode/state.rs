//! Everything the `OpenCode` verbs decide from, read once per run, and the plan
//! they return: the steps to report and the whole-file writes to make.

use std::path::{Path, PathBuf};

use toolu_runtime::env::Env;

use crate::fail::Res;
use crate::opencode::entries::{PluginEntry, toolu_entries};
use crate::opencode::jsonc::{ConfigFile, read_config_file};
use crate::opencode::paths::{Paths, Scope, paths, project_root_of};
use crate::opencode::selection::{SelectionFile, read_selection_file, skills_disabled};
use crate::step::Step;

/// A whole-file write a verb plans; a dry run prints it instead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Write {
  /// The file.
  pub(crate) path: PathBuf,
  /// Its new text.
  pub(crate) text: String,
}

/// What a verb decided.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Plan {
  /// The steps to report.
  pub(crate) steps: Vec<Step>,
  /// The files to write.
  pub(crate) writes: Vec<Write>,
}

/// The project's `OpenCode` files as read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct State {
  /// Where they are.
  pub(crate) paths: Paths,
  /// The global config files, ascending priority.
  pub(crate) global: Vec<ConfigFile>,
  /// The project config files, load order.
  pub(crate) project: Vec<ConfigFile>,
  /// The global selection.
  pub(crate) selection_global: SelectionFile,
  /// The project selection.
  pub(crate) selection_project: SelectionFile,
  /// Plugins `skills.<name>: false` turns off.
  pub(crate) disabled: Vec<String>,
}

/// Reads the state of the project containing `cwd`.
pub(crate) fn load(cwd: &Path, env: &Env) -> Res<State> {
  let root = project_root_of(cwd, env);
  let paths = paths(&root, env, cwd);
  let read = |files: &[PathBuf]| {
    files
      .iter()
      .map(|path| read_config_file(path))
      .collect::<Res<Vec<_>>>()
  };
  Ok(State {
    global: read(&paths.global_files)?,
    project: read(&paths.project_files)?,
    selection_global: read_selection_file(&paths.selection_global)?,
    selection_project: read_selection_file(&paths.selection_project)?,
    disabled: skills_disabled(&paths.toolu_config),
    paths,
  })
}

impl State {
  /// The config files of `scope`.
  pub(crate) fn files(&self, scope: Scope) -> &[ConfigFile] {
    match scope {
      Scope::Global => &self.global,
      Scope::Project => &self.project,
    }
  }

  /// The enabled list of `scope`'s selection, `None` without a file.
  pub(crate) fn selection(&self, scope: Scope) -> Option<&[String]> {
    let file = match scope {
      Scope::Global => &self.selection_global,
      Scope::Project => &self.selection_project,
    };
    file.enabled.as_deref()
  }

  /// The selection an edit without `--scope` lands in: the project's when it
  /// exists, since that is the one `@toolu/opencode` reads there.
  pub(crate) fn default_selection_scope(&self) -> Scope {
    if self.selection(Scope::Project).is_some() {
      Scope::Project
    } else {
      Scope::Global
    }
  }

  /// The selection governing `scope`: a project falls back to the global one.
  pub(crate) fn governing_selection(&self, scope: Scope) -> Option<&[String]> {
    match scope {
      Scope::Global => self.selection(Scope::Global),
      Scope::Project => self
        .selection(Scope::Project)
        .or_else(|| self.selection(Scope::Global)),
    }
  }

  /// The toolu entries of `scope`.
  pub(crate) fn entries_in(&self, scope: Scope) -> Res<Vec<PluginEntry>> {
    let mut all = Vec::new();
    for file in self.files(scope) {
      all.extend(toolu_entries(file)?);
    }
    Ok(all)
  }

  /// A note for a global selection edit that a project selection overrides here.
  pub(crate) fn overridden_note(&self, scope: Scope) -> String {
    if scope != Scope::Global || self.selection(Scope::Project).is_none() {
      return String::new();
    }
    format!(
      "; no effect in this project: {} governs it",
      self.paths.selection_project.display()
    )
  }

  /// The scopes holding a toolu entry.
  pub(crate) fn installed_scopes(&self) -> Res<Vec<Scope>> {
    let mut scopes = Vec::new();
    for scope in [Scope::Global, Scope::Project] {
      if !self.entries_in(scope)?.is_empty() {
        scopes.push(scope);
      }
    }
    Ok(scopes)
  }

  /// Whether `path` is one of the `OpenCode` config files (not a selection).
  pub(crate) fn is_config(&self, path: &Path) -> bool {
    self
      .global
      .iter()
      .chain(&self.project)
      .any(|file| file.path == path)
  }

  /// The text `path` held when the state was read (`None` inside: absent), or
  /// `None` for a path the state never read.
  pub(crate) fn original(&self, path: &Path) -> Option<&Option<String>> {
    let configs = self.global.iter().chain(&self.project);
    if let Some(file) = configs.into_iter().find(|file| file.path == path) {
      return Some(&file.text);
    }
    [&self.selection_global, &self.selection_project]
      .into_iter()
      .zip([&self.paths.selection_global, &self.paths.selection_project])
      .find(|(_, selection_path)| selection_path.as_path() == path)
      .map(|(file, _)| &file.text)
  }
}

#[cfg(test)]
#[path = "tests/state_test.rs"]
mod tests;
