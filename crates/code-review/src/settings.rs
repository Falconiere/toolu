//! The `codeReview` section of `toolu.config.json` (#666). `enabled` is the
//! per-repository opt-in: it is read from the project file only, so a user-level
//! setting can never send code from every repository an agent opens. `effort` is a
//! preference, read from the merged user and project config.

use std::path::Path;

use serde_json::Value;
use toolu_runtime::config::load::{LoadedConfig, check_text, is_file, load as load_config};
use toolu_runtime::host::roots::Roots;

use crate::budget::Effort;

/// The config section this plugin owns.
const SECTION: &str = "codeReview";

/// What a run without the opt-in tells the agent; it sends nothing.
pub const ENABLE_HINT: &str = "code review is not enabled for this repository, so nothing was sent; ask the user to opt in with \"codeReview\": {\"enabled\": true} in the project toolu.config.json (a user-level setting cannot enable it)";

/// The plugin's settings for one repository.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Settings {
  /// The project config sets `codeReview.enabled: true`.
  pub enabled: bool,
  /// `codeReview.effort`, when it names a level.
  pub effort: Option<Effort>,
  /// Why a setting was ignored.
  pub notes: Vec<String>,
}

/// The settings for the project found from `cwd`. An invalid config envelope
/// fails closed: not enabled.
pub fn load(roots: &Roots, cwd: Option<&Path>) -> Settings {
  from_config(&load_config(roots, cwd))
}

/// The settings in an already loaded config.
pub fn from_config(config: &LoadedConfig) -> Settings {
  let mut settings = Settings::default();
  if let Some(reason) = &config.invalid {
    settings.notes.push(format!(
      "invalid config ({reason}); code review stays disabled"
    ));
    return settings;
  }
  settings.enabled = config.files.project.as_deref().is_some_and(project_enabled);
  match config
    .data
    .get(SECTION)
    .and_then(|section| section.get("effort"))
  {
    None => {}
    Some(Value::String(name)) if Effort::parse(name).is_some() => {
      settings.effort = Effort::parse(name);
    }
    Some(_) => settings.notes.push(format!(
      "{SECTION}.effort is not one of auto|small|medium|high|xhigh; ignored"
    )),
  }
  settings
}

/// Whether the project file itself holds `codeReview.enabled: true`.
fn project_enabled(path: &Path) -> bool {
  if !is_file(path) {
    return false;
  }
  let Ok(text) = std::fs::read_to_string(path) else {
    return false;
  };
  check_text(&text).is_ok_and(|map| {
    map
      .get(SECTION)
      .and_then(|section| section.get("enabled"))
      .and_then(Value::as_bool)
      == Some(true)
  })
}

#[cfg(test)]
#[path = "tests/settings_test.rs"]
mod tests;
