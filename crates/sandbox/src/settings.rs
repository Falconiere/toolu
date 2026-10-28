//! The `sandbox` section of `toolu.config.json`: which backend runs commands, and
//! the VM's name, size, image and extra mounts. The config loader only knows the
//! key; this module refuses unknown or invalid values, naming the setting, and
//! fills the rest from the machine's facts.

use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use crate::facts::{Facts, Os};

/// Where commands run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Backend {
  /// A Lima VM (the default on macOS).
  Lima,
  /// This machine itself (the default on Linux).
  Host,
}

impl Backend {
  /// The name the config and `--json` use.
  pub(crate) fn name(self) -> &'static str {
    match self {
      Backend::Lima => "lima",
      Backend::Host => "host",
    }
  }

  fn parse(name: &str) -> Option<Backend> {
    match name {
      "lima" => Some(Backend::Lima),
      "host" => Some(Backend::Host),
      _ => None,
    }
  }
}

/// The settings as written; unset keys are `None`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Settings {
  backend: Option<Backend>,
  name: Option<String>,
  cpus: Option<u32>,
  memory: Option<u32>,
  disk: Option<u32>,
  template: Option<String>,
  mounts: Vec<PathBuf>,
}

/// Every setting with its value, defaults filled in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Resolved {
  /// Where commands run.
  pub(crate) backend: Backend,
  /// The VM's name.
  pub(crate) name: String,
  /// The VM's CPUs.
  pub(crate) cpus: u32,
  /// The VM's memory in GiB.
  pub(crate) memory_gib: u32,
  /// The VM's disk in GiB.
  pub(crate) disk_gib: u32,
  /// A Lima template replacing the pinned Debian 13 image, when set.
  pub(crate) template: Option<String>,
  /// Host directories mounted besides the project.
  pub(crate) mounts: Vec<PathBuf>,
}

/// The VM's name unless `sandbox.name` sets one.
pub(crate) const DEFAULT_NAME: &str = "toolu-sandbox";
const DEFAULT_DISK_GIB: u32 = 100;
const MIN_MEMORY_GIB: u32 = 2;
const MAX_DEFAULT_MEMORY_GIB: u32 = 32;
const MIN_CPUS: u32 = 2;
/// The cores left to the host by default.
const HOST_CORES: u32 = 2;
/// The settings that decide where commands run, which Lima VM that is (the name
/// picks an instance toolu then edits and boots, whoever created it) and what
/// they can reach. They come from the user's own config file, never from a
/// project's checked-in one, which a cloned repository controls.
const USER_ONLY: [&str; 4] = ["backend", "name", "mounts", "template"];
const KEYS: [&str; 7] = [
  "backend", "name", "cpus", "memory", "disk", "template", "mounts",
];

/// `merged` with the [`USER_ONLY`] settings taken from `user` alone, so a value a
/// project file set for them is dropped.
pub(crate) fn user_only_keys(
  merged: Option<&Map<String, Value>>,
  user: &Map<String, Value>,
) -> Option<Map<String, Value>> {
  let mut section = merged?.clone();
  for key in USER_ONLY {
    section.remove(key);
    if let Some(value) = user.get(key) {
      section.insert(key.to_owned(), value.clone());
    }
  }
  Some(section)
}

/// The settings in `section`, with `~` in mounts meaning `home`.
///
/// # Errors
/// A message naming the first unknown key or invalid value.
pub(crate) fn parse(section: Option<&Map<String, Value>>, home: &Path) -> Result<Settings, String> {
  let Some(section) = section else {
    return Ok(Settings::default());
  };
  if let Some(key) = section.keys().find(|key| !KEYS.contains(&key.as_str())) {
    return Err(format!(
      "sandbox.{key} is not a setting; the settings are {}",
      KEYS.join(", ")
    ));
  }
  Ok(Settings {
    backend: read(section, "backend", "\"lima\" or \"host\"", |value| {
      value.as_str().and_then(Backend::parse)
    })?,
    name: read(
      section,
      "name",
      "1 to 63 lowercase letters, digits or dashes, starting with a letter or digit",
      |value| {
        value
          .as_str()
          .filter(|name| valid_name(name))
          .map(str::to_owned)
      },
    )?,
    cpus: read(section, "cpus", "a whole number from 1 to 256", |value| {
      whole(value, 1, 256)
    })?,
    memory: read(section, "memory", "whole GiB from 1 to 1024", |value| {
      whole(value, 1, 1024)
    })?,
    disk: read(section, "disk", "whole GiB from 10 to 8192", |value| {
      whole(value, 10, 8192)
    })?,
    template: read(section, "template", "a non-empty string", |value| {
      value
        .as_str()
        .filter(|text| !text.trim().is_empty())
        .map(str::to_owned)
    })?,
    mounts: mounts(section.get("mounts"), home)?,
  })
}

/// `settings` with every unset value filled in from `facts`.
pub(crate) fn resolve(settings: Settings, facts: &Facts) -> Resolved {
  let backend = settings.backend.unwrap_or(match facts.os {
    Os::Macos => Backend::Lima,
    Os::Linux | Os::Other => Backend::Host,
  });
  Resolved {
    backend,
    name: settings.name.unwrap_or_else(|| DEFAULT_NAME.to_owned()),
    cpus: settings
      .cpus
      .unwrap_or_else(|| facts.cores.saturating_sub(HOST_CORES).max(MIN_CPUS)),
    memory_gib: settings
      .memory
      .unwrap_or_else(|| (facts.memory_gib / 2).clamp(MIN_MEMORY_GIB, MAX_DEFAULT_MEMORY_GIB)),
    disk_gib: settings.disk.unwrap_or(DEFAULT_DISK_GIB),
    template: settings.template,
    mounts: settings.mounts,
  }
}

/// `section[key]`, unset when absent or `null`, read by `value`.
fn read<T>(
  section: &Map<String, Value>,
  key: &str,
  want: &str,
  value: impl Fn(&Value) -> Option<T>,
) -> Result<Option<T>, String> {
  match section.get(key) {
    None | Some(Value::Null) => Ok(None),
    Some(found) => value(found)
      .map(Some)
      .ok_or_else(|| format!("sandbox.{key} must be {want}, not {found}")),
  }
}

fn whole(value: &Value, low: u64, high: u64) -> Option<u32> {
  value
    .as_u64()
    .filter(|number| (low..=high).contains(number))
    .and_then(|number| u32::try_from(number).ok())
}

/// A Lima instance name: `[a-z0-9][a-z0-9-]*`, at most 63 characters.
fn valid_name(name: &str) -> bool {
  let mut chars = name.chars();
  let first = chars
    .next()
    .is_some_and(|first| first.is_ascii_lowercase() || first.is_ascii_digit());
  first
    && name.len() <= 63
    && chars.all(|char| char.is_ascii_lowercase() || char.is_ascii_digit() || char == '-')
}

fn mounts(value: Option<&Value>, home: &Path) -> Result<Vec<PathBuf>, String> {
  let want = "a list of absolute paths or paths starting with ~/";
  let items = match value {
    None | Some(Value::Null) => return Ok(Vec::new()),
    Some(Value::Array(items)) => items,
    Some(other) => return Err(format!("sandbox.mounts must be {want}, not {other}")),
  };
  items
    .iter()
    .map(|item| {
      item
        .as_str()
        .and_then(|text| expand(text, home))
        .ok_or_else(|| format!("sandbox.mounts must be {want}, not {item}"))
    })
    .collect()
}

fn expand(text: &str, home: &Path) -> Option<PathBuf> {
  if text == "~" {
    return Some(home.to_path_buf());
  }
  if let Some(rest) = text.strip_prefix("~/") {
    return Some(home.join(rest));
  }
  Path::new(text).is_absolute().then(|| PathBuf::from(text))
}

#[cfg(test)]
#[path = "tests/settings_test.rs"]
mod tests;
