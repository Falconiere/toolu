//! `cargo xtask gate-coverage`: live hooks match the coverage inventory.

use std::path::Path;

use serde::Deserialize;

use crate::gate_coverage_discover::discover;
use crate::options::Options;
use crate::{Verdict, output};

const CLASSIFICATIONS: [&str; 4] = ["shell-out", "port-native", "port-new", "no-map"];

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Row {
  pub(crate) id: String,
  pub(crate) source_path: String,
  pub(crate) plugin: String,
  pub(crate) kind: String,
  pub(crate) event: String,
  pub(crate) matcher: String,
  pub(crate) command_or_module: String,
  pub(crate) host_mechanism: String,
  pub(crate) parent_id: Option<String>,
  #[serde(default)]
  pub(crate) semantics: String,
  #[serde(default)]
  pub(crate) classification: String,
  #[serde(default)]
  pub(crate) support: String,
  #[serde(default)]
  pub(crate) implementation_issue: Option<u64>,
  #[serde(default)]
  pub(crate) implementation_status: String,
  #[serde(default)]
  pub(crate) limits: String,
  #[serde(default)]
  pub(crate) bash_required: bool,
}

/// Compare discovery with `fixtures/gate-coverage/inventory.json` and the matrix.
pub(crate) fn run(options: &Options) -> Result<Verdict, String> {
  let found = check(&options.root)?;
  if found.is_empty() {
    output::say("gate-coverage-inventory: ok");
    return Ok(Verdict::Clean);
  }
  for problem in &found {
    output::error(&format!("gate-coverage-inventory: {problem}"));
  }
  Ok(Verdict::Findings)
}

pub(crate) fn check(root: &Path) -> Result<Vec<String>, String> {
  let discovered = match discover(root) {
    Ok(rows) => rows,
    Err(message) => return Ok(vec![message]),
  };
  let inventory = match load_inventory(root) {
    Ok(rows) => rows,
    Err(message) => return Ok(vec![message]),
  };
  let mut errors = id_gaps(&discovered, &inventory);
  errors.extend(field_gaps(&discovered, &inventory));
  for row in &inventory {
    errors.extend(validate_row(row));
  }
  let matrix_path = root.join("docs/gate-coverage-matrix.md");
  if !matrix_path.is_file() {
    return Ok(vec![format!("missing matrix {}", matrix_path.display())]);
  }
  let matrix = std::fs::read_to_string(&matrix_path)
    .map_err(|err| format!("cannot read {}: {err}", matrix_path.display()))?;
  for row in &inventory {
    if !matrix.contains(&row.id) {
      errors.push(format!("matrix missing id: {}", row.id));
    }
  }
  Ok(errors)
}

fn id_gaps(discovered: &[Row], inventory: &[Row]) -> Vec<String> {
  let mut errors = Vec::new();
  for row in discovered {
    if !inventory.iter().any(|item| item.id == row.id) {
      errors.push(format!("missing from inventory: {}", row.id));
    }
  }
  for row in inventory {
    if !discovered.iter().any(|item| item.id == row.id) {
      errors.push(format!("orphan inventory id: {}", row.id));
    }
  }
  errors
}

fn field_gaps(discovered: &[Row], inventory: &[Row]) -> Vec<String> {
  const KEYS: [&str; 8] = [
    "sourcePath",
    "plugin",
    "kind",
    "event",
    "matcher",
    "commandOrModule",
    "hostMechanism",
    "parentId",
  ];
  let mut errors = Vec::new();
  for row in inventory {
    let Some(live) = discovered.iter().find(|item| item.id == row.id) else {
      continue;
    };
    for key in KEYS {
      if field(row, key) != field(live, key) {
        errors.push(format!("{}: {key} differs from discovery", row.id));
      }
    }
  }
  errors
}

fn field(row: &Row, key: &str) -> String {
  match key {
    "sourcePath" => row.source_path.clone(),
    "plugin" => row.plugin.clone(),
    "kind" => row.kind.clone(),
    "event" => row.event.clone(),
    "matcher" => row.matcher.clone(),
    "commandOrModule" => row.command_or_module.clone(),
    "hostMechanism" => row.host_mechanism.clone(),
    "parentId" => row.parent_id.clone().unwrap_or_default(),
    _ => String::new(),
  }
}

fn validate_row(row: &Row) -> Vec<String> {
  let mut errors = validate_identity(row);
  errors.extend(validate_status(row));
  errors
}

fn validate_identity(row: &Row) -> Vec<String> {
  let mut errors = Vec::new();
  if row.id.is_empty() {
    errors.push("row missing id".to_owned());
  }
  if row.id.contains(['"', '\'']) {
    errors.push(format!("{}: id must not contain quote characters", row.id));
  }
  if row.command_or_module.contains(['"', '\'']) {
    errors.push(format!(
      "{}: commandOrModule must not contain quote characters",
      row.id
    ));
  }
  if !CLASSIFICATIONS.contains(&row.classification.as_str()) {
    errors.push(format!(
      "{}: invalid classification {}",
      row.id, row.classification
    ));
  }
  errors
}

fn validate_status(row: &Row) -> Vec<String> {
  let mut errors = Vec::new();
  if row.classification != "port-native" {
    errors.push(format!(
      "{}: final inventory requires classification=port-native",
      row.id
    ));
  }
  if row.bash_required {
    errors.push(format!(
      "{}: final inventory requires bashRequired=false",
      row.id
    ));
  }
  if row.implementation_status != "done" {
    errors.push(format!(
      "{}: final inventory requires implementationStatus=done",
      row.id
    ));
  }
  if row.classification == "no-map" && row.limits.trim().is_empty() {
    errors.push(format!("{}: no-map requires limits rationale", row.id));
  }
  if row.support == "n/a" && row.classification != "no-map" {
    errors.push(format!(
      "{}: support=n/a requires classification=no-map",
      row.id
    ));
  }
  if row.support == "required" && row.implementation_issue.is_none() {
    errors.push(format!(
      "{}: required row needs implementationIssue",
      row.id
    ));
  }
  errors
}

fn load_inventory(root: &Path) -> Result<Vec<Row>, String> {
  let rel = "fixtures/gate-coverage/inventory.json";
  let path = root.join(rel);
  if !path.is_file() {
    return Err(format!("missing inventory {}", path.display()));
  }
  let text = std::fs::read_to_string(&path).map_err(|err| format!("cannot read {rel}: {err}"))?;
  serde_json::from_str(&text).map_err(|err| format!("inventory schema invalid: {err}"))
}

#[cfg(test)]
#[path = "tests/gate_coverage_test.rs"]
mod tests;
