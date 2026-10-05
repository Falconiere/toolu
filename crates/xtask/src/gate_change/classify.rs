//! What one changed path is: gate data (with a reason), product code, or neither.

use std::path::Path;

use serde_json::Value;

use super::additive::additive;
use crate::data::{self, Rules};

/// The classification of one path.
#[derive(Debug, Default, PartialEq)]
pub(super) struct Change {
  /// Why the change touches gate data, if it does.
  pub(super) gate: Option<String>,
  /// Whether the change touches product code.
  pub(super) product: bool,
}

/// Files whose every change is gate data.
const GATE_FILES: &[&str] = &["clippy.toml", "rustfmt.toml", "deny.toml"];

/// Data files whose every change is gate data.
const GATE_DATA: &[&str] = &["rules.json", "jscpd.json"];

/// Data files that may grow next to product code.
const REGISTRATION: &[&str] = &["layers.json", "folders.json", "inventory.json"];

/// Host configs that hold `lang.rust`.
const CONFIGS: &[&str] = &[".claude/toolu.config.json", ".codex/toolu.config.json"];

/// Classify `path`, given its text at the base and now (`None` when absent).
pub(super) fn classify(
  path: &Path,
  before: Option<&str>,
  after: Option<&str>,
  root: &Path,
) -> Result<Change, String> {
  let name = path.to_string_lossy().replace('\\', "/");
  let gate = |reason: String| Change {
    gate: Some(reason),
    product: false,
  };
  if name == "Cargo.toml" {
    return Ok(manifest(before, after));
  }
  if GATE_FILES.contains(&name.as_str()) {
    return Ok(gate(format!("{name} changed")));
  }
  if CONFIGS.contains(&name.as_str()) {
    let rust = |text: Option<&str>| json(text).pointer("/lang/rust").cloned();
    return Ok(if rust(before) == rust(after) {
      Change::default()
    } else {
      gate(format!("{name} lang.rust changed"))
    });
  }
  if let Some(file) = name.strip_prefix(&format!("{}/", data::DATA_DIR)) {
    return data_file(file, before, after, root);
  }
  Ok(Change {
    gate: None,
    product: name.starts_with("crates/") || name == "Cargo.lock",
  })
}

fn json(text: Option<&str>) -> Value {
  text
    .and_then(|text| serde_json::from_str(text).ok())
    .unwrap_or(Value::Null)
}

/// The root manifest: `[workspace.lints]` is gate data, the rest is product.
fn manifest(before: Option<&str>, after: Option<&str>) -> Change {
  let parse = |text: Option<&str>| -> toml::Table {
    text.and_then(|text| text.parse().ok()).unwrap_or_default()
  };
  let (mut old, mut new) = (parse(before), parse(after));
  let lints = |table: &mut toml::Table| {
    table
      .get_mut("workspace")
      .and_then(toml::Value::as_table_mut)
      .and_then(|workspace| workspace.remove("lints"))
  };
  let lints_changed = lints(&mut old) != lints(&mut new);
  Change {
    gate: lints_changed.then(|| "Cargo.toml [workspace.lints] changed".to_owned()),
    product: old != new,
  }
}

fn data_file(
  file: &str,
  before: Option<&str>,
  after: Option<&str>,
  root: &Path,
) -> Result<Change, String> {
  let shown = format!("{}/{file}", data::DATA_DIR);
  let reason = if GATE_DATA.contains(&file) {
    Some(format!("{shown} changed"))
  } else if REGISTRATION.contains(&file) {
    (!additive(&json(before), &json(after)))
      .then(|| format!("{shown} changed an existing entry (only additions are registration)"))
  } else if file == "coverage-floor.json" {
    floors(&json(before), &json(after), &data::rules(root)?)
  } else {
    Some(format!("{shown} is new gate data"))
  };
  Ok(Change {
    gate: reason,
    product: false,
  })
}

/// A removed or lowered row, or a new row below the default floor.
fn floors(before: &Value, after: &Value, rules: &Rules) -> Option<String> {
  let rows = |value: &Value| {
    value
      .get("floors")
      .and_then(Value::as_object)
      .cloned()
      .unwrap_or_default()
  };
  let (old, new) = (rows(before), rows(after));
  for (krate, was) in &old {
    match new.get(krate).and_then(Value::as_f64) {
      None => return Some(format!("coverage floor of {krate} removed")),
      Some(now) if was.as_f64().is_some_and(|was| now < was) => {
        return Some(format!("coverage floor of {krate} lowered to {now}"));
      }
      Some(_) => {}
    }
  }
  new.iter().find_map(|(krate, now)| {
    let now = now.as_f64().unwrap_or(0.0);
    let default = rules
      .coverage
      .strict
      .get(krate)
      .copied()
      .unwrap_or(rules.coverage.default);
    (!old.contains_key(krate) && now < default)
      .then(|| format!("coverage floor of {krate} added at {now}, below the default {default}"))
  })
}

#[cfg(test)]
#[path = "tests/classify_test.rs"]
mod tests;
