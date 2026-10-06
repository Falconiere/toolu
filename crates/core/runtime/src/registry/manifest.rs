//! The module manifest, `<spec>__<name>.json` in an event directory: what each
//! plugin's `SessionStart` writes to enable a compiled-in rule. Strict: an
//! unknown field or a version other than 1 is rejected, since a format change
//! bumps `hookProtocol`.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::{ModuleKind, NameParse, RegistryEvent, parse_name};

/// `{"version":1,"spec":…,"name":…,"event":"tool/pre"|"tool/post","matcher":…}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleManifest {
  /// Always 1.
  pub version: u32,
  /// The owning plugin, `name@marketplace`; equals the file name's spec.
  pub spec: String,
  /// The rule name; equals the file name's name.
  pub name: String,
  /// The event; equals the directory's.
  pub event: RegistryEvent,
  /// `*` for every tool, or `|`-separated entries, each an exact tool name or
  /// a prefix ending in `*` (`mcp__*`).
  pub matcher: String,
}

impl ModuleManifest {
  /// Whether the rule runs for `tool`.
  pub fn matches(&self, tool: &str) -> bool {
    self
      .matcher
      .split('|')
      .any(|entry| match entry.strip_suffix('*') {
        Some(prefix) => tool.starts_with(prefix),
        None => entry == tool,
      })
  }
}

/// Reads and checks the manifest at `path` in `event`'s directory.
///
/// # Errors
/// `<path>: <reason>` when the file cannot be read or parsed, has an unknown
/// field, a version other than 1, a spec, name or event that differ from its
/// file name and directory, or an empty matcher entry.
pub fn read_manifest(path: &Path, event: RegistryEvent) -> Result<ModuleManifest, String> {
  let fail = |reason: String| format!("{}: {reason}", path.display());
  let text = std::fs::read_to_string(path).map_err(|err| fail(err.to_string()))?;
  let manifest: ModuleManifest =
    serde_json::from_str(&text).map_err(|err| fail(err.to_string()))?;
  if manifest.version != 1 {
    return Err(fail(format!(
      "unsupported version {} (supported: 1)",
      manifest.version
    )));
  }
  let base = path
    .file_name()
    .map(|name| name.to_string_lossy().into_owned())
    .unwrap_or_default();
  let named = match parse_name(&base) {
    NameParse::Module(parsed) if parsed.kind == ModuleKind::Manifest => parsed,
    NameParse::Module(_) | NameParse::NotModule | NameParse::Unnamespaced => {
      return Err(fail("not a <spec>__<name>.json file".to_owned()));
    }
  };
  if named.spec != manifest.spec || named.name != manifest.name {
    return Err(fail(format!(
      "names {}__{}, not the file's {}__{}",
      manifest.spec, manifest.name, named.spec, named.name
    )));
  }
  if manifest.event != event {
    return Err(fail(format!(
      "event {} in the {} directory",
      manifest.event.slug(),
      event.dir_name()
    )));
  }
  if manifest.matcher.split('|').any(str::is_empty) {
    return Err(fail("an empty matcher entry".to_owned()));
  }
  Ok(manifest)
}

#[cfg(test)]
#[path = "tests/manifest_test.rs"]
mod tests;
