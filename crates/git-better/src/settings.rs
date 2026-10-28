//! The `gitBetter` section of `toolu.config.json`. User and project settings
//! merge with project precedence. Any value the plugin cannot read is reported
//! and replaced by its default, so enforcement never silently turns off.

use std::path::Path;

use serde_json::{Map, Value};
use toolu_runtime::config::load::{LoadedConfig, load as load_config};
use toolu_runtime::config::settings::{COMMIT_PREFIXES, read_list, settings_dir};
use toolu_runtime::host::roots::Roots;

/// The config section this plugin owns.
pub(crate) const SECTION: &str = "gitBetter";

/// The Conventional Commits types of `plugins/toolu/settings/commit-prefixes.txt`,
/// for when that file cannot be found.
pub(crate) const DEFAULT_TYPES: [&str; 11] = [
  "feat", "fix", "chore", "docs", "refactor", "test", "perf", "build", "ci", "style", "revert",
];

/// What the rule does with a commit or branch that is not semantic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
  /// Deny the command and explain the fix.
  Block,
  /// Let it run and explain the fix.
  Advise,
  /// Check nothing.
  Off,
}

impl Mode {
  fn parse(name: &str) -> Option<Mode> {
    match name {
      "block" => Some(Mode::Block),
      "advise" => Some(Mode::Advise),
      "off" => Some(Mode::Off),
      _ => None,
    }
  }
}

/// The plugin's settings for one repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Settings {
  /// `gitBetter.mode`: `block` by default.
  pub(crate) mode: Mode,
  /// `gitBetter.requireScope`: a subject needs `(scope)`; on by default.
  pub(crate) require_scope: bool,
  /// `gitBetter.requireRef`: a branch needs an issue reference; off by default.
  pub(crate) require_ref: bool,
  /// `gitBetter.allowWip`: `wip` commits off the base branch; on by default.
  pub(crate) allow_wip: bool,
  /// The commit types: `gitBetter.types`, else `commit-prefixes.txt`, else [`DEFAULT_TYPES`].
  pub(crate) types: Vec<String>,
  /// Why a value was ignored.
  pub(crate) notes: Vec<String>,
}

impl Default for Settings {
  fn default() -> Self {
    Settings {
      mode: Mode::Block,
      require_scope: true,
      require_ref: false,
      allow_wip: true,
      types: DEFAULT_TYPES
        .iter()
        .map(|kind| (*kind).to_owned())
        .collect(),
      notes: Vec::new(),
    }
  }
}

/// The settings for the project found from `cwd`; `plugin_root` locates the
/// toolu settings directory that holds `commit-prefixes.txt`.
pub(crate) fn load(roots: &Roots, cwd: Option<&Path>, plugin_root: Option<&Path>) -> Settings {
  let prefixes = settings_dir(roots, plugin_root)
    .and_then(|dir| read_list(&dir.join(COMMIT_PREFIXES)).ok())
    .unwrap_or_default();
  from_config(&load_config(roots, cwd), prefixes)
}

fn flag(section: &Map<String, Value>, key: &str, value: &mut bool, notes: &mut Vec<String>) {
  match section.get(key) {
    None => {}
    Some(Value::Bool(set)) => *value = *set,
    Some(_) => notes.push(format!("{SECTION}.{key} must be a boolean; ignored")),
  }
}

fn types(section: &Map<String, Value>, notes: &mut Vec<String>) -> Option<Vec<String>> {
  let list = section.get("types")?;
  let read: Option<Vec<String>> = list.as_array().and_then(|items| {
    items
      .iter()
      .map(|item| {
        item
          .as_str()
          .filter(|kind| valid_type(kind))
          .map(str::to_owned)
      })
      .collect()
  });
  let kinds = read.filter(|kinds| !kinds.is_empty());
  if kinds.is_none() {
    notes.push(format!(
      "{SECTION}.types must be a non-empty array of lowercase words; ignored"
    ));
  }
  kinds
}

fn valid_type(kind: &str) -> bool {
  !kind.is_empty() && kind.bytes().all(|byte| byte.is_ascii_lowercase())
}

/// The settings in an already loaded config, with the types `prefixes` names.
pub(crate) fn from_config(config: &LoadedConfig, prefixes: Vec<String>) -> Settings {
  let mut settings = Settings::default();
  let prefixes: Vec<String> = prefixes
    .into_iter()
    .filter(|kind| valid_type(kind))
    .collect();
  if !prefixes.is_empty() {
    settings.types = prefixes;
  }
  if let Some(reason) = &config.invalid {
    let note = format!("invalid config ({reason}); {SECTION} uses its defaults");
    settings.notes.push(note);
    return settings;
  }
  let section = match config.data.get(SECTION) {
    None | Some(Value::Null) => return settings,
    Some(Value::Object(section)) => section,
    Some(_) => {
      let note = format!("{SECTION} must be an object; its defaults apply");
      settings.notes.push(note);
      return settings;
    }
  };
  let notes = &mut settings.notes;
  match section
    .get("mode")
    .map(|mode| mode.as_str().and_then(Mode::parse))
  {
    None => {}
    Some(Some(mode)) => settings.mode = mode,
    Some(None) => notes.push(format!(
      "{SECTION}.mode is not one of block|advise|off; ignored"
    )),
  }
  flag(section, "requireScope", &mut settings.require_scope, notes);
  flag(section, "requireRef", &mut settings.require_ref, notes);
  flag(section, "allowWip", &mut settings.allow_wip, notes);
  if let Some(kinds) = types(section, notes) {
    settings.types = kinds;
  }
  settings
}

#[cfg(test)]
#[path = "tests/settings_test.rs"]
mod tests;
