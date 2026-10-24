//! The plugin catalog: the marketplace manifest's plugins and their
//! dependencies. The binary carries the repository's
//! `.claude-plugin/marketplace.json` from its build, so `toolu plugins` works
//! with no checkout beside it; `--catalog <path>` reads another, which is how the
//! npm wrapper hands over the copy it ships.

use std::path::Path;

use serde_json::Value;

use crate::fail::{Fail, Res};

/// Dependency order over the catalog.
pub(crate) mod order;

/// The manifest this binary was built with.
const EMBEDDED: &str = toolu_runtime::catalog::BUILT_IN;

/// One catalog plugin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Entry {
  /// Its plugin name.
  pub(crate) name: String,
  /// One line for the plugin picker.
  pub(crate) description: Option<String>,
  /// The names it depends on, in manifest order.
  pub(crate) dependencies: Vec<String>,
}

/// A validated marketplace manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Catalog {
  /// Every plugin, in manifest order.
  pub(crate) plugins: Vec<Entry>,
}

impl Catalog {
  /// The plugin names, in manifest order.
  pub(crate) fn names(&self) -> Vec<String> {
    self
      .plugins
      .iter()
      .map(|entry| entry.name.clone())
      .collect()
  }

  /// The plugin called `name`.
  pub(crate) fn entry(&self, name: &str) -> Option<&Entry> {
    self.plugins.iter().find(|entry| entry.name == name)
  }

  /// Whether `name` can be a plugin the marketplace retired (#615): it is not
  /// in this catalog and is shaped like a plugin name (lowercase letters and
  /// digits, single inner hyphens), so it never names a path.
  pub(crate) fn retired(&self, name: &str) -> bool {
    let shaped = !name.is_empty()
      && name.split('-').all(|part| {
        !part.is_empty()
          && part
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
      });
    shaped && self.entry(name).is_none()
  }
}

/// The catalog at `path`, or the built-in one.
pub(crate) fn load(path: Option<&Path>) -> Res<Catalog> {
  let Some(path) = path else {
    return parse(EMBEDDED, "the built-in catalog");
  };
  let label = path.display().to_string();
  let text = std::fs::read_to_string(path)
    .map_err(|_unreadable| Fail::failed(format!("cannot read marketplace manifest: {label}")))?;
  parse(&text, &label)
}

/// Validates manifest `text`; `source` names it in errors.
pub(crate) fn parse(text: &str, source: &str) -> Res<Catalog> {
  let value: Value = serde_json::from_str(text).map_err(|err| {
    Fail::usage(format!(
      "marketplace manifest is not valid JSON: {source}: {err}"
    ))
  })?;
  let malformed = |detail: &str| {
    Fail::usage(format!(
      "marketplace manifest is malformed: {source}: {detail}"
    ))
  };
  let name = value.get("name").and_then(Value::as_str);
  if name.is_none_or(str::is_empty) {
    return Err(malformed("\"name\" must be a non-empty string"));
  }
  let plugins = value
    .get("plugins")
    .and_then(Value::as_array)
    .filter(|plugins| !plugins.is_empty())
    .ok_or_else(|| malformed("\"plugins\" must be a non-empty array"))?;
  let plugins = plugins
    .iter()
    .enumerate()
    .map(|(index, plugin)| {
      entry(plugin).map_err(|detail| malformed(&format!("plugins[{index}]: {detail}")))
    })
    .collect::<Result<Vec<_>, _>>()?;
  let catalog = Catalog { plugins };
  // A dependency cycle is refused here, so no later walk meets one.
  order::install_order(&catalog, &[])?;
  Ok(catalog)
}

fn entry(plugin: &Value) -> Result<Entry, String> {
  let name = plugin
    .get("name")
    .and_then(Value::as_str)
    .filter(|name| !name.is_empty())
    .ok_or("\"name\" must be a non-empty string")?;
  let description = match plugin.get("description") {
    None => None,
    Some(Value::String(text)) => Some(text.clone()),
    Some(_other) => return Err("\"description\" must be a string".to_owned()),
  };
  let dependencies = match plugin.get("dependencies") {
    None => Vec::new(),
    Some(Value::Array(items)) => items.iter().map(dependency).collect::<Result<_, _>>()?,
    Some(_other) => return Err("\"dependencies\" must be an array".to_owned()),
  };
  Ok(Entry {
    name: name.to_owned(),
    description,
    dependencies,
  })
}

fn dependency(item: &Value) -> Result<String, String> {
  item
    .get("name")
    .and_then(Value::as_str)
    .map(str::to_owned)
    .ok_or_else(|| "a dependency must be an object with a string \"name\"".to_owned())
}

#[cfg(test)]
#[path = "tests/catalog_test.rs"]
mod tests;
