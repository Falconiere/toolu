//! `install --config <path>`: replay a `.toolu/plugins.json` selection with the
//! rules of the TypeScript reader (the former TypeScript installer's `src/config/read.ts`): an
//! unknown format version or a file from a newer major is refused, not guessed at.

use std::path::Path;

use serde_json::Value;

use crate::fail::{Fail, Res};
use crate::host::Target;

/// The only selection-file format this binary reads.
const VERSION: u64 = 1;

/// A replayed selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Selection {
  /// The advisory host; `--host` wins.
  pub(crate) host: Option<Target>,
  /// The plugins to install.
  pub(crate) enabled: Vec<String>,
}

fn major(version: &str) -> u64 {
  version
    .split('.')
    .next()
    .and_then(|major| major.parse().ok())
    .unwrap_or(0)
}

/// Reads and checks the selection at `path` for a binary at `version`.
pub(crate) fn read(path: &Path, version: &str) -> Res<Selection> {
  let label = path.display();
  let text = std::fs::read_to_string(path)
    .map_err(|_unreadable| Fail::usage(format!("cannot read selection file: {label}")))?;
  let value: Value = serde_json::from_str(&text)
    .map_err(|err| Fail::usage(format!("{label} is not valid JSON: {err}")))?;
  let Some(declared) = value.get("version").and_then(Value::as_f64) else {
    return Err(Fail::usage(format!(
      "{label} has no numeric \"version\" field"
    )));
  };
  if value.get("version").and_then(Value::as_u64) != Some(VERSION) {
    return Err(Fail::usage(format!(
      "{label} declares version {declared}; this CLI understands version {VERSION}"
    )));
  }
  let selection =
    parse(&value).map_err(|detail| Fail::usage(format!("{label} is malformed: {detail}")))?;
  if let Some(written) = value.get("generatorVersion").and_then(Value::as_str)
    && major(written) > major(version)
  {
    return Err(Fail::usage(format!(
      "{label} was written by toolu {written}; this CLI is {version}. Upgrade the CLI."
    )));
  }
  Ok(selection)
}

fn parse(value: &Value) -> Result<Selection, String> {
  match value.get("generatorVersion") {
    None => {}
    Some(Value::String(text)) if !text.is_empty() => {}
    Some(_other) => return Err("\"generatorVersion\" must be a non-empty string".to_owned()),
  }
  let host = match value.get("host") {
    None => None,
    Some(Value::String(name)) => Some(match name.as_str() {
      "claude" => Target::Claude,
      "codex" => Target::Codex,
      "opencode" => Target::Opencode,
      _other => {
        return Err(format!(
          "\"host\" must be claude, codex or opencode, got {name}"
        ));
      }
    }),
    Some(_other) => return Err("\"host\" must be a string".to_owned()),
  };
  let enabled = value
    .get("enabled")
    .and_then(Value::as_array)
    .ok_or("\"enabled\" must be an array of plugin names")?
    .iter()
    .map(|item| {
      item
        .as_str()
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .ok_or("\"enabled\" must hold non-empty strings".to_owned())
    })
    .collect::<Result<Vec<_>, _>>()?;
  Ok(Selection { host, enabled })
}

#[cfg(test)]
#[path = "tests/replay_test.rs"]
mod tests;
