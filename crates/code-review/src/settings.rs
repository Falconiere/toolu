//! The `codeReview` section of `toolu.config.json`. Review is enabled by default;
//! user and project settings merge with project precedence. Invalid config fails
//! closed before any outbound request.

use std::path::Path;

use serde_json::Value;
use toolu_runtime::config::load::{LoadedConfig, check_text, load as load_config};
use toolu_runtime::host::roots::Roots;

use crate::budget::Effort;

/// The config section this plugin owns.
const SECTION: &str = "codeReview";

/// What a disabled run tells the agent; it sends nothing.
pub(crate) const ENABLE_HINT: &str = "code review is disabled by configuration, so nothing was sent; inspect codeReview.enabled in user and project toolu.config.json (project settings take precedence)";

/// The plugin's settings for one repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
  /// Enabled unless the merged config explicitly disables it.
  pub enabled: bool,
  /// `codeReview.effort`, when it names a level.
  pub effort: Option<Effort>,
  /// Why a setting was ignored.
  pub notes: Vec<String>,
}

impl Default for Settings {
  fn default() -> Self {
    Self {
      enabled: true,
      effort: None,
      notes: Vec::new(),
    }
  }
}

/// The settings for the project found from `cwd`. An invalid config envelope
/// fails closed: not enabled.
pub fn load(roots: &Roots, cwd: Option<&Path>) -> Settings {
  from_config(&load_config(roots, cwd))
}

/// The settings in an already loaded config.
fn from_config(config: &LoadedConfig) -> Settings {
  let mut settings = Settings::default();
  if let Some(reason) = &config.invalid {
    settings.enabled = false;
    settings.notes.push(format!(
      "invalid config ({reason}); code review stays disabled"
    ));
    return settings;
  }
  if let Some(reason) = malformed_config(config) {
    settings.enabled = false;
    settings.notes.push(reason);
    return settings;
  }
  settings.enabled = enabled(config, &mut settings.notes);
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

/// Keep the review fail-closed even though the shared loader ignores malformed files.
fn malformed_config(config: &LoadedConfig) -> Option<String> {
  for path in [Some(&config.files.user), config.files.project.as_ref()]
    .into_iter()
    .flatten()
  {
    let checked = match std::fs::metadata(path) {
      Ok(metadata) if metadata.is_file() => std::fs::read_to_string(path)
        .map_err(|err| err.to_string())
        .and_then(|text| check_text(&text)),
      Ok(_) => Err("config path is not a regular file".to_owned()),
      Err(error)
        if error.kind() == std::io::ErrorKind::NotFound
          && std::fs::symlink_metadata(path)
            .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound) =>
      {
        continue;
      }
      Err(error) => Err(error.to_string()),
    };
    if let Err(reason) = checked {
      return Some(format!(
        "invalid config ({}: {reason}); code review stays disabled",
        path.display()
      ));
    }
  }
  None
}

/// Enablement from the merged user and project setting, rejecting invalid values.
fn enabled(config: &LoadedConfig, notes: &mut Vec<String>) -> bool {
  match config.data.get(SECTION) {
    None => true,
    Some(Value::Object(section)) => match section.get("enabled") {
      None => true,
      Some(Value::Bool(enabled)) => *enabled,
      Some(_) => {
        notes.push("codeReview.enabled must be a boolean; code review stays disabled".to_owned());
        false
      }
    },
    Some(_) => {
      notes.push("codeReview must be an object; code review stays disabled".to_owned());
      false
    }
  }
}

#[cfg(test)]
#[path = "tests/settings_test.rs"]
mod tests;
