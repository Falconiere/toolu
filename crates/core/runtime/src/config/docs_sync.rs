//! Docs-sync globs (`docs-sync-config.ts`): `docsSync.<key>`, an array in the
//! merged config, replaces the built-in list; an empty or non-array override
//! keeps the default.

use serde_json::Value;

use super::load::LoadedConfig;
use super::read::section;
use crate::json::stringify_pretty;

/// A docs-sync list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocsSyncKey {
  /// Doc files whose change satisfies the check.
  Surfaces,
  /// Doc paths carved back out of the surfaces.
  SurfaceExcludes,
  /// Code files whose change demands a doc touch.
  CodeSurfaces,
}

impl DocsSyncKey {
  /// The `docsSync.<key>` name.
  pub fn name(self) -> &'static str {
    match self {
      DocsSyncKey::Surfaces => "surfaces",
      DocsSyncKey::SurfaceExcludes => "surfaceExcludes",
      DocsSyncKey::CodeSurfaces => "codeSurfaces",
    }
  }

  /// The built-in list.
  pub fn defaults(self) -> &'static [&'static str] {
    match self {
      DocsSyncKey::Surfaces => &[
        "README.md",
        "*/README.md",
        "docs/*.md",
        "*/SKILL.md",
        "AGENTS.md",
        "*/AGENTS.md",
        "CLAUDE.md",
        "*/CLAUDE.md",
        "*/workflows/*.md",
      ],
      DocsSyncKey::SurfaceExcludes => &["docs/releases/*", "*/docs/releases/*"],
      DocsSyncKey::CodeSurfaces => &[
        "*.ts",
        "*.rs",
        "*.sh",
        "*/commands/*",
        "*plugin.json",
        "*.config.json",
      ],
    }
  }
}

/// jq `-r .[]` captured by `$(...)`: one line per element, non-strings
/// pretty-printed, trailing newlines dropped.
fn jq_raw_lines(items: &[Value]) -> Vec<String> {
  let rendered: Vec<String> = items
    .iter()
    .map(|item| {
      item
        .as_str()
        .map_or_else(|| stringify_pretty(item), str::to_owned)
    })
    .collect();
  let text = rendered.join("\n");
  let text = text.trim_end_matches('\n');
  if text.is_empty() {
    Vec::new()
  } else {
    text.split('\n').map(str::to_owned).collect()
  }
}

/// The `docsSync.<key>` globs, or the built-in list.
pub fn docs_sync_globs(config: &LoadedConfig, key: DocsSyncKey) -> Vec<String> {
  let configured = section(config, "docsSync").and_then(|docs| docs.get(key.name()));
  let lines = configured
    .and_then(Value::as_array)
    .map(|items| jq_raw_lines(items))
    .unwrap_or_default();
  if lines.is_empty() {
    key
      .defaults()
      .iter()
      .map(|glob| (*glob).to_owned())
      .collect()
  } else {
    lines
  }
}

#[cfg(test)]
#[path = "tests/docs_sync_test.rs"]
mod tests;
