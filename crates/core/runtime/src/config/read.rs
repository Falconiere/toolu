//! Config readers (`config-read.ts`): the `config.sh` public API over a loaded
//! config. Each mirrors its jq filter, including how a wrong-typed section or
//! value falls back, so resolved values match bash on the same files.

use serde_json::{Map, Value};

use super::load::LoadedConfig;
use crate::host::roots::CallerError;
use crate::json::stringify;

/// The model tiers, in routing order.
pub const MODEL_CLASSES: [&str; 6] = [
  "mechanical",
  "exploration",
  "implementation",
  "review",
  "synthesis",
  "architecture",
];

/// The model aliases a tier may name.
pub const MODEL_ALIASES: [&str; 5] = ["haiku", "sonnet", "opus", "fable", "inherit"];

/// The reasoning efforts a Codex tier may name.
pub const CODEX_REASONING_EFFORTS: [&str; 6] = ["low", "medium", "high", "xhigh", "max", "ultra"];

/// Each tier's default alias, in [`MODEL_CLASSES`] order.
const MODEL_DEFAULTS: [&str; 6] = ["haiku", "sonnet", "sonnet", "sonnet", "opus", "opus"];

/// Each tier's default Codex model and effort, in [`MODEL_CLASSES`] order.
const CODEX_DEFAULTS: [(&str, &str); 6] = [
  ("gpt-5.6-luna", "medium"),
  ("gpt-5.6-terra", "medium"),
  ("gpt-5.6-terra", "medium"),
  ("gpt-5.6-terra", "high"),
  ("gpt-5.6-sol", "high"),
  ("gpt-5.6-sol", "high"),
];

/// A Codex tier's model slug and reasoning effort.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexModel {
  /// The model slug.
  pub model: String,
  /// One of [`CODEX_REASONING_EFFORTS`].
  pub reasoning_effort: String,
}

/// `(.[$key]? // {})`: the section when it is an object.
pub fn section<'a>(config: &'a LoadedConfig, key: &str) -> Option<&'a Map<String, Value>> {
  config.data.get(key).and_then(Value::as_object)
}

fn member<'a>(config: &'a LoadedConfig, category: &str, name: &str) -> Option<&'a Value> {
  section(config, category)?.get(name)
}

/// `toolu_enabled`: on unless the value is `false` or the string `"false"`.
pub fn enabled(config: &LoadedConfig, category: &str, name: &str) -> bool {
  let value = member(config, category, name);
  value != Some(&Value::Bool(false)) && !is_text(value, "false")
}

/// `toolu_flag_true`: on only for the JSON boolean `true`.
pub fn flag_true(config: &LoadedConfig, category: &str, name: &str) -> bool {
  member(config, category, name) == Some(&Value::Bool(true))
}

/// `toolu_flag_false`: an explicit opt-out, the JSON boolean `false` only.
pub fn flag_false(config: &LoadedConfig, category: &str, name: &str) -> bool {
  member(config, category, name) == Some(&Value::Bool(false))
}

/// `toolu_enabled_explicit`: off unless the value is `true` or the string `"true"`.
pub fn enabled_explicit(config: &LoadedConfig, category: &str, name: &str) -> bool {
  let value = member(config, category, name);
  value == Some(&Value::Bool(true)) || is_text(value, "true")
}

fn is_text(value: Option<&Value>, text: &str) -> bool {
  value.and_then(Value::as_str) == Some(text)
}

/// jq `getpath`: a missing key or null ends the walk; indexing a scalar or an
/// array is a jq error, which bash turns into the default too.
fn read_path<'a>(data: &'a Map<String, Value>, path: &str) -> Option<&'a Value> {
  let mut keys = path.split('.');
  let first = data.get(keys.next()?)?;
  let found = keys.try_fold(first, |current, key| current.as_object()?.get(key))?;
  (!found.is_null()).then_some(found)
}

/// `toolu_string PATH DEFAULT ALLOWED...`: the string at dotted `path` when it
/// is one of `allowed`. Absent falls back silently; present but unqualified
/// warns, then falls back.
pub fn config_string(
  config: &LoadedConfig,
  path: &str,
  fallback: &str,
  allowed: &[&str],
) -> String {
  let Some(value) = read_path(&config.data, path) else {
    return fallback.to_owned();
  };
  let Some(text) = value.as_str() else {
    config.warn(format!("{path}: value is not a string; using {fallback}"));
    return fallback.to_owned();
  };
  if allowed.contains(&text) {
    return text.to_owned();
  }
  let allowed = allowed.join(" ");
  config.warn(format!(
    "{path}: '{text}' is not an allowed value ({allowed}); using {fallback}"
  ));
  fallback.to_owned()
}

fn class_index(class: &str) -> Result<usize, CallerError> {
  MODEL_CLASSES
    .iter()
    .position(|known| *known == class)
    .ok_or_else(|| {
      let known = MODEL_CLASSES.join(" ");
      CallerError(format!("unknown model class '{class}' (known: {known})"))
    })
}

/// jq `tostring`: strings as they are, anything else as compact JSON.
fn jq_to_string(value: &Value) -> String {
  value
    .as_str()
    .map_or_else(|| stringify(value), str::to_owned)
}

/// `toolu_model CLASS`: `.models.<class>` when it is an alias, else the default.
///
/// # Errors
/// [`CallerError`] for a class outside [`MODEL_CLASSES`].
pub fn model(config: &LoadedConfig, class: &str) -> Result<String, CallerError> {
  let index = class_index(class)?;
  let fallback = MODEL_DEFAULTS.get(index).copied().unwrap_or("inherit");
  let value = section(config, "models").and_then(|models| models.get(class));
  // jq `// ""` also swallows false, and an empty string means unset.
  let text = match value {
    None | Some(Value::Null | Value::Bool(false)) => return Ok(fallback.to_owned()),
    Some(value) => jq_to_string(value),
  };
  if text.is_empty() {
    return Ok(fallback.to_owned());
  }
  if MODEL_ALIASES.contains(&text.as_str()) {
    return Ok(text);
  }
  let aliases = MODEL_ALIASES.join(" ");
  config.warn(format!(
    "models.{class}: '{text}' is not a model alias ({aliases}); using {fallback}"
  ));
  Ok(fallback.to_owned())
}

/// `.models.codex.<class>.<field>` through objects only; `None` when
/// unreachable, null or false.
fn codex_field<'a>(config: &'a LoadedConfig, class: &str, field: &str) -> Option<&'a Value> {
  let entry = section(config, "models")?
    .get("codex")?
    .as_object()?
    .get(class)?;
  let value = entry.as_object()?.get(field)?;
  (!matches!(value, Value::Null | Value::Bool(false))).then_some(value)
}

/// `toolu_codex_model CLASS`: model slug and reasoning effort, each falling back alone.
///
/// # Errors
/// [`CallerError`] for a class outside [`MODEL_CLASSES`].
pub fn codex_model(config: &LoadedConfig, class: &str) -> Result<CodexModel, CallerError> {
  let index = class_index(class)?;
  let (model, effort) = CODEX_DEFAULTS.get(index).copied().unwrap_or(("", "medium"));
  let prefix = format!("models.codex.{class}");
  let slug = match codex_field(config, class, "model") {
    Some(Value::String(slug)) if !slug.is_empty() => slug.clone(),
    None => model.to_owned(),
    Some(_) => {
      config.warn(format!(
        "{prefix}.model: value is not a non-empty string; using {model}"
      ));
      model.to_owned()
    }
  };
  let reasoning_effort = match codex_field(config, class, "reasoningEffort") {
    None => effort.to_owned(),
    Some(Value::String(raw)) if CODEX_REASONING_EFFORTS.contains(&raw.as_str()) => raw.clone(),
    Some(Value::String(raw)) => {
      let efforts = CODEX_REASONING_EFFORTS.join(" ");
      config.warn(format!(
        "{prefix}.reasoningEffort: '{raw}' is not supported ({efforts}); using {effort}"
      ));
      effort.to_owned()
    }
    Some(_) => {
      config.warn(format!(
        "{prefix}.reasoningEffort: value is not a string; using {effort}"
      ));
      effort.to_owned()
    }
  };
  Ok(CodexModel {
    model: slug,
    reasoning_effort,
  })
}

#[cfg(test)]
#[path = "tests/read_test.rs"]
mod tests;
