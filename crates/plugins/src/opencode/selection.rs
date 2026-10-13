//! The plugin selection `@toolu/opencode` reads (`toolu/plugins.json`,
//! `{ "version": 1, "enabled": [...] }`) and the enabled set it implies: the
//! listed catalog names, minus `skills.<name>: false`, closed over dependencies.

use std::path::Path;

use serde_json::Value;

use crate::catalog::Catalog;
use crate::catalog::order::install_order;
use crate::fail::{Fail, Res};

/// A selection file's text and its enabled list; both `None` when absent.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct SelectionFile {
  /// The text as read, to detect a concurrent change before a write.
  pub(crate) text: Option<String>,
  /// The enabled names.
  pub(crate) enabled: Option<Vec<String>>,
}

/// Reads a selection file exactly as the adapter's schema does (strict:
/// `version` 1 and `enabled` only); an invalid file fails closed.
pub(crate) fn read_selection_file(path: &Path) -> Res<SelectionFile> {
  let label = path.display();
  let text = match std::fs::read_to_string(path) {
    Ok(text) => text,
    Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(SelectionFile::default()),
    Err(err) => return Err(Fail::failed(format!("cannot read {label}: {err}"))),
  };
  let invalid =
    |detail: String| Fail::failed(format!("invalid {label}: {detail}; nothing was written"));
  let value: Value = serde_json::from_str(&text).map_err(|err| invalid(err.to_string()))?;
  let object = value
    .as_object()
    .ok_or_else(|| invalid("expected an object".to_owned()))?;
  if let Some(key) = object
    .keys()
    .find(|key| *key != "version" && *key != "enabled")
  {
    return Err(invalid(format!("unrecognized key \"{key}\"")));
  }
  if object.get("version").and_then(Value::as_u64) != Some(1) {
    return Err(invalid("\"version\" must be 1".to_owned()));
  }
  let enabled = object
    .get("enabled")
    .and_then(Value::as_array)
    .ok_or_else(|| invalid("\"enabled\" must be an array".to_owned()))?
    .iter()
    .map(|item| {
      item
        .as_str()
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
    })
    .collect::<Option<Vec<_>>>()
    .ok_or_else(|| invalid("\"enabled\" must hold non-empty strings".to_owned()))?;
  Ok(SelectionFile {
    text: Some(text),
    enabled: Some(enabled),
  })
}

/// The text of a selection enabling `enabled`, as `JSON.stringify(_, null, 2)`
/// writes it (`version` first).
pub(crate) fn selection_text(enabled: &[String]) -> String {
  let names: Vec<String> = enabled
    .iter()
    .map(|name| format!("    {}", serde_json::to_string(name).unwrap_or_default()))
    .collect();
  let list = if names.is_empty() {
    "[]".to_owned()
  } else {
    format!("[\n{}\n  ]", names.join(",\n"))
  };
  format!("{{\n  \"version\": 1,\n  \"enabled\": {list}\n}}\n")
}

/// Plugins turned off by `skills.<name>: false` in the project's toolu config;
/// like the adapter, an unreadable or malformed config disables nothing.
pub(crate) fn skills_disabled(path: &Path) -> Vec<String> {
  let Ok(text) = std::fs::read_to_string(path) else {
    return Vec::new();
  };
  let Ok(value) = serde_json::from_str::<Value>(&text) else {
    return Vec::new();
  };
  let Some(skills) = value.get("skills").and_then(Value::as_object) else {
    return Vec::new();
  };
  if skills.values().any(|flag| !flag.is_boolean()) {
    return Vec::new();
  }
  skills
    .iter()
    .filter(|(_, flag)| flag.as_bool() == Some(false))
    .map(|(name, _)| name.clone())
    .collect()
}

/// `names` plus every catalog plugin they need, in install order; names outside
/// the catalog are dropped.
pub(crate) fn closure(catalog: &Catalog, names: &[String]) -> Vec<String> {
  let seeds: Vec<String> = names
    .iter()
    .filter(|name| catalog.entry(name).is_some())
    .cloned()
    .collect();
  if seeds.is_empty() {
    return Vec::new();
  }
  install_order(catalog, &seeds).unwrap_or_default()
}

/// What the adapter enables: listed catalog names not disabled by skills, closed.
pub(crate) fn effective_enabled(
  catalog: &Catalog,
  listed: Option<&[String]>,
  disabled: &[String],
) -> Vec<String> {
  let names = listed.map_or_else(|| catalog.names(), <[String]>::to_vec);
  let kept: Vec<String> = names
    .into_iter()
    .filter(|name| !disabled.contains(name))
    .collect();
  closure(catalog, &kept)
}

/// Enabled plugins outside `removing` that need `name`, directly or not.
pub(crate) fn dependents_blocking(
  catalog: &Catalog,
  enabled: &[String],
  removing: &[String],
  name: &str,
) -> Vec<String> {
  enabled
    .iter()
    .filter(|other| *other != name && !removing.contains(other))
    .filter(|other| {
      closure(catalog, std::slice::from_ref(other))
        .iter()
        .any(|needed| needed == name)
    })
    .cloned()
    .collect()
}

#[cfg(test)]
#[path = "tests/selection_test.rs"]
mod tests;
