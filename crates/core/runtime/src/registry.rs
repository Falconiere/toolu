//! Registry types (`packages/toolu-core/src/registry/registry-{types,paths}.ts`):
//! the events a module subscribes to, where each event's modules live, the
//! `<spec>__<name>.<ext>` file names, the module manifest and the `Rule` trait.
//! They sit below both the registry runner (#418) and startup, which form a
//! cycle in TypeScript.

pub mod manifest;
pub mod rule;

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::host::roots::{CallerError, Roots};

/// A canonical event a registry module subscribes to; `tool/pre` modules also see `shell/pre`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RegistryEvent {
  /// Before a tool call.
  #[serde(rename = "tool/pre")]
  ToolPre,
  /// After a tool call.
  #[serde(rename = "tool/post")]
  ToolPost,
}

impl RegistryEvent {
  /// Both events.
  pub const ALL: [RegistryEvent; 2] = [RegistryEvent::ToolPre, RegistryEvent::ToolPost];

  /// `tool/pre` or `tool/post`.
  pub fn slug(self) -> &'static str {
    match self {
      RegistryEvent::ToolPre => "tool/pre",
      RegistryEvent::ToolPost => "tool/post",
    }
  }

  /// The directory its modules live in: `pre-tools.d` or `post-tools.d`.
  pub fn dir_name(self) -> &'static str {
    match self {
      RegistryEvent::ToolPre => "pre-tools.d",
      RegistryEvent::ToolPost => "post-tools.d",
    }
  }
}

/// What a registry file is, by its extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleKind {
  /// `.js`: a bundled ESM module, run through the Bun bridge until #440.
  Esm,
  /// `.sh`: an executable module.
  Bash,
  /// `.json`: the manifest of a compiled-in rule.
  Manifest,
}

impl ModuleKind {
  /// The file extension, without the dot.
  pub fn extension(self) -> &'static str {
    match self {
      ModuleKind::Esm => "js",
      ModuleKind::Bash => "sh",
      ModuleKind::Manifest => "json",
    }
  }
}

/// A registry file name split into its parts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedName {
  /// The owning plugin, `name@marketplace`.
  pub spec: String,
  /// The module name.
  pub name: String,
  /// What the file is.
  pub kind: ModuleKind,
}

/// How a base name in a registry directory reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NameParse {
  /// Not a `.js`, `.sh` or `.json` file: ignored.
  NotModule,
  /// A module file without a `<spec>__` namespace.
  Unnamespaced,
  /// A namespaced module file.
  Module(ParsedName),
}

/// The separator between spec and name; specs may contain `.` but never `__`.
const SEP: &str = "__";

/// `<config root>/toolu` (not created).
pub fn registry_root(roots: &Roots) -> PathBuf {
  roots.config_root().join("toolu")
}

/// `<root>/pre-tools.d` or `<root>/post-tools.d` (not created).
pub fn event_dir(roots: &Roots, event: RegistryEvent) -> PathBuf {
  registry_root(roots).join(event.dir_name())
}

/// `<spec>__<name>.<ext>`.
///
/// # Errors
/// [`CallerError`] for a spec or name that could not round-trip through [`parse_name`].
pub fn file_name(spec: &str, name: &str, kind: ModuleKind) -> Result<String, CallerError> {
  if spec.is_empty()
    || spec.chars().any(char::is_whitespace)
    || spec.contains('/')
    || spec.contains(SEP)
  {
    return Err(CallerError(format!(
      "registry: invalid plugin spec {spec:?}"
    )));
  }
  if name.is_empty() || name.contains('/') || name.starts_with('.') {
    return Err(CallerError(format!(
      "registry: invalid module name {name:?}"
    )));
  }
  Ok(format!("{spec}{SEP}{name}.{}", kind.extension()))
}

/// Splits a registry base name as `dispatch.sh` did: the spec runs to the first
/// `__` and must be non-empty and free of white space.
pub fn parse_name(base: &str) -> NameParse {
  let kinds = [ModuleKind::Esm, ModuleKind::Bash, ModuleKind::Manifest];
  let found = kinds.into_iter().find_map(|kind| {
    let stem = base.strip_suffix(kind.extension())?.strip_suffix('.')?;
    Some((stem, kind))
  });
  let Some((stem, kind)) = found else {
    return NameParse::NotModule;
  };
  match stem.split_once(SEP) {
    Some((spec, name)) if !spec.is_empty() && !spec.chars().any(char::is_whitespace) => {
      NameParse::Module(ParsedName {
        spec: spec.to_owned(),
        name: name.to_owned(),
        kind,
      })
    }
    Some(_) | None => NameParse::Unnamespaced,
  }
}

#[cfg(test)]
#[path = "tests/registry_test.rs"]
mod tests;
