//! Quality thresholds (`quality-config.ts`). Each limit resolves the override
//! (`lang.<lang>.<key>`), then the native linter config (TypeScript
//! `maxFileLines` only: the active linter's `max-lines`), then the built-in
//! default. A layer that cannot produce a positive number falls through;
//! nothing here warns.

use std::path::{Path, PathBuf};

use serde_json::Value;

use super::load::{LoadedConfig, is_file};
use super::read::section;
use crate::cli_args::is_number_text;
use crate::env::Env;
use crate::git;
use crate::json::is_js_space;

/// A language with quality thresholds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QualityLang {
  /// TypeScript.
  Ts,
  /// Rust.
  Rust,
  /// Python.
  Python,
}

impl QualityLang {
  /// The `lang.<name>` key.
  pub fn name(self) -> &'static str {
    match self {
      QualityLang::Ts => "ts",
      QualityLang::Rust => "rust",
      QualityLang::Python => "python",
    }
  }

  /// The built-in limits, as `(key, value)`.
  pub fn defaults(self) -> &'static [(&'static str, u64)] {
    match self {
      QualityLang::Ts => &[("maxFileLines", 300), ("maxFnLines", 60)],
      QualityLang::Rust => &[
        ("maxFileLines", 500),
        ("maxFnLines", 50),
        ("maxImplLines", 200),
      ],
      QualityLang::Python => &[("maxFileLines", 400), ("maxFnLines", 50)],
    }
  }
}

/// Where a resolved limit came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThresholdSource {
  /// `lang.<lang>.<key>`.
  Override,
  /// The active linter's config.
  Native,
  /// The built-in default.
  Default,
}

/// A positive float's floor as an integer; past `u64` it saturates.
fn floor_u64(number: f64) -> u64 {
  number.floor().to_string().parse().unwrap_or(u64::MAX)
}

/// jq `if string then tonumber? else . end | if number and > 0 then floor`.
fn positive_floor(value: &Value) -> Option<u64> {
  let number = match value {
    Value::String(text) => {
      let trimmed = text.trim_matches(is_js_space);
      is_number_text(trimmed)
        .then(|| trimmed.parse::<f64>().ok())
        .flatten()?
    }
    Value::Number(number) => number.as_f64()?,
    Value::Null | Value::Bool(_) | Value::Array(_) | Value::Object(_) => return None,
  };
  (number > 0.0).then(|| floor_u64(number))
}

fn lang_member<'a>(config: &'a LoadedConfig, lang: QualityLang, key: &str) -> Option<&'a Value> {
  section(config, "lang")?
    .get(lang.name())?
    .as_object()?
    .get(key)
}

/// The `max-lines` rule of a parsed eslint or oxlint config: `N`, `["error", N]`
/// or `["error", {"max": N}]`; severity `"off"` or `0` and non-positive values
/// give `None`.
pub fn native_max_lines(linter_config: &Value) -> Option<u64> {
  let rule = linter_config.get("rules")?.as_object()?.get("max-lines")?;
  match rule {
    Value::Array(items) => {
      let severity = items.first();
      if severity.and_then(Value::as_str) == Some("off")
        || severity.and_then(Value::as_f64) == Some(0.0)
      {
        return None;
      }
      let option = items.get(1)?;
      positive_floor(option.get("max").unwrap_or(option))
    }
    Value::Number(_) | Value::String(_) => positive_floor(rule),
    Value::Null | Value::Bool(_) | Value::Object(_) => None,
  }
}

/// `detect_ts_linter`'s machine-readable config: biome, then oxc, then eslint.
fn active_linter_config(root: &Path) -> Option<PathBuf> {
  if is_file(&root.join("biome.json")) || is_file(&root.join("biome.jsonc")) {
    return None;
  }
  let oxlint = root.join(".oxlintrc.json");
  if is_file(&oxlint) {
    return Some(oxlint);
  }
  let names = std::fs::read_dir(root).ok()?;
  let eslint = names.flatten().any(|entry| {
    let name = entry.file_name().to_string_lossy().into_owned();
    name.starts_with(".eslintrc") || name.starts_with("eslint.config.")
  });
  eslint.then(|| root.join(".eslintrc.json"))
}

/// `root`, else the git toplevel of the process directory.
fn project_root(root: Option<&Path>) -> Option<PathBuf> {
  match root {
    Some(root) => Some(root.to_path_buf()),
    None => git::toplevel(&Env::process(), &std::env::current_dir().ok()?),
  }
}

fn native_ts_max_lines(root: Option<&Path>) -> Option<u64> {
  let file = active_linter_config(&project_root(root)?).filter(|file| is_file(file))?;
  let text = std::fs::read_to_string(file).ok()?;
  native_max_lines(&serde_json::from_str(&text).ok()?)
}

/// `ts_max_file_lines_resolved`: the TypeScript file limit and its layer;
/// `root` is the project whose linter config is read, the git toplevel of the
/// process directory when `None`.
pub fn ts_max_file_lines_resolved(
  config: &LoadedConfig,
  root: Option<&Path>,
) -> (u64, ThresholdSource) {
  if let Some(value) = lang_member(config, QualityLang::Ts, "maxFileLines").and_then(positive_floor)
  {
    return (value, ThresholdSource::Override);
  }
  match native_ts_max_lines(root) {
    Some(value) => (value, ThresholdSource::Native),
    None => (300, ThresholdSource::Default),
  }
}

/// `quality_threshold`: always a number; bash floors, so a 0.5 override gives 0.
/// An unknown key gives 0.
pub fn quality_threshold(
  config: &LoadedConfig,
  lang: QualityLang,
  key: &str,
  root: Option<&Path>,
) -> u64 {
  if let Some(value) = lang_member(config, lang, key).and_then(positive_floor) {
    return value;
  }
  if lang == QualityLang::Ts && key == "maxFileLines" {
    return ts_max_file_lines_resolved(config, root).0;
  }
  let default = lang.defaults().iter().find(|(name, _)| *name == key);
  default.map_or(0, |(_, value)| *value)
}

/// `quality_flag`: a literal JSON boolean at `lang.<lang>.<key>`, else `fallback`.
pub fn quality_flag(config: &LoadedConfig, lang: QualityLang, key: &str, fallback: bool) -> bool {
  lang_member(config, lang, key)
    .and_then(Value::as_bool)
    .unwrap_or(fallback)
}

#[cfg(test)]
#[path = "tests/quality_test.rs"]
mod tests;
