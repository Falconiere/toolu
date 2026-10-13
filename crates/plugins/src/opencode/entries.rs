//! The `@toolu/opencode` entries of `OpenCode`'s `plugin` arrays, and the entry
//! the pinned host actually loads once it merges its config files.

use serde_json::Value;
use toolu_runtime::env::Env;

use crate::fail::{Fail, Res};
use crate::opencode::jsonc::ConfigFile;

/// The one npm package that carries every plugin on `OpenCode`.
pub(crate) const PACKAGE: &str = "@toolu/opencode";

/// A toolu element of a file's `plugin` array: a spec or `[spec, options]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PluginEntry {
  /// The file it is in.
  pub(crate) file: String,
  /// Its index in the array.
  pub(crate) index: usize,
  /// The spec string.
  pub(crate) spec: String,
  /// Whether it is a `[spec, options]` tuple.
  pub(crate) tuple: bool,
}

/// The npm package of a spec; URLs and paths come back whole and never match.
pub(crate) fn package_name(spec: &str) -> &str {
  let from = if spec.starts_with('@') {
    spec.find('/').unwrap_or(0)
  } else {
    0
  };
  let skip = from.max(1);
  match spec.get(skip..).and_then(|rest| rest.find('@')) {
    Some(at) => spec.get(..skip + at).unwrap_or(spec),
    None => spec,
  }
}

/// The version part of a spec, `latest` when unpinned.
pub(crate) fn version_of(spec: &str) -> &str {
  let name = package_name(spec);
  spec
    .get(name.len() + 1..)
    .filter(|_| spec.len() > name.len())
    .unwrap_or("latest")
}

/// The spec install writes: `TOOLU_OPENCODE_PACKAGE`, else the package at `version`.
pub(crate) fn target_spec(env: &Env, version: &str) -> Res<String> {
  match env.get("TOOLU_OPENCODE_PACKAGE") {
    None => Ok(format!("{PACKAGE}@{version}")),
    Some(spec) if package_name(spec) == PACKAGE => Ok(spec.to_owned()),
    Some(spec) => Err(Fail::usage(format!(
      "TOOLU_OPENCODE_PACKAGE must name {PACKAGE}, got: {spec}"
    ))),
  }
}

/// The file's `plugin` array, `None` when the key is absent; a `plugin` that
/// is not an array is an error naming the file.
pub(crate) fn plugin_array(file: &ConfigFile) -> Res<Option<&Vec<Value>>> {
  match file.data.as_ref().and_then(|data| data.get("plugin")) {
    None => Ok(None),
    Some(Value::Array(items)) => Ok(Some(items)),
    Some(_other) => Err(Fail::failed(format!(
      "{}: \"plugin\" must be an array; nothing was written",
      file.path.display()
    ))),
  }
}

/// The length of the file's `plugin` array, `None` when absent.
pub(crate) fn plugin_len(file: &ConfigFile) -> Res<Option<usize>> {
  Ok(plugin_array(file)?.map(Vec::len))
}

/// The file's array, one slot per element in order: the entry of an element
/// that names a spec, `None` for any other element.
fn elements(file: &ConfigFile) -> Res<Vec<Option<PluginEntry>>> {
  let items = plugin_array(file)?.map(Vec::as_slice).unwrap_or_default();
  Ok(
    items
      .iter()
      .enumerate()
      .map(|(index, item)| {
        let tuple_spec = || {
          item
            .as_array()
            .and_then(|tuple| tuple.first())
            .and_then(Value::as_str)
        };
        let (spec, tuple) = match (item.as_str(), tuple_spec()) {
          (Some(spec), _) => (spec, false),
          (None, Some(spec)) => (spec, true),
          (None, None) => return None,
        };
        Some(PluginEntry {
          file: file.path.display().to_string(),
          index,
          spec: spec.to_owned(),
          tuple,
        })
      })
      .collect(),
  )
}

fn is_toolu(entry: &PluginEntry) -> bool {
  package_name(&entry.spec) == PACKAGE
}

/// The toolu entries of one file.
pub(crate) fn toolu_entries(file: &ConfigFile) -> Res<Vec<PluginEntry>> {
  Ok(
    elements(file)?
      .into_iter()
      .flatten()
      .filter(is_toolu)
      .collect(),
  )
}

/// The highest-priority global file defining `plugin` (files in ascending priority).
pub(crate) fn governing_global(global: &[ConfigFile]) -> Res<Option<&ConfigFile>> {
  for file in global.iter().rev() {
    if plugin_array(file)?.is_some() {
      return Ok(Some(file));
    }
  }
  Ok(None)
}

/// The toolu entry `OpenCode` loads: the governing global array, then each
/// project file in load order; an empty array resets the list, and the last
/// entry for the package wins.
pub(crate) fn effective_entry(
  global: &[ConfigFile],
  project: &[ConfigFile],
) -> Res<Option<PluginEntry>> {
  let mut merged = match governing_global(global)? {
    Some(file) => elements(file)?,
    None => Vec::new(),
  };
  for file in project {
    match plugin_len(file)? {
      None => {}
      Some(0) => merged.clear(),
      Some(_) => merged.extend(elements(file)?),
    }
  }
  Ok(merged.into_iter().flatten().rfind(is_toolu))
}

#[cfg(test)]
#[path = "tests/entries_test.rs"]
mod tests;
