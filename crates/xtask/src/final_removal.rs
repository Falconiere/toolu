//! `cargo xtask final-removal`: the Bun-only cutover still holds.

use std::path::Path;
use std::process::Command;

use regex::Regex;
use serde::Deserialize;
use serde_json::Value;

use crate::options::Options;
use crate::{Verdict, output};

const SHELL_KEEP: [&str; 2] = ["install.sh", "plugins/jev/scripts/jev.sh"];
const HOSTS: [&str; 2] = ["bun-bundle", "native"];
const SHELL_SCRIPTS: [&str; 3] = ["lint:shell", "test:shell", "test:shell:serial"];

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Row {
  id: String,
  classification: String,
  host_mechanism: String,
  implementation_status: String,
  bash_required: bool,
}

/// Judge tracked shell, the workflow, and the native coverage inventory.
pub(crate) fn run(options: &Options) -> Result<Verdict, String> {
  let found = problems(&options.root)?;
  if found.is_empty() {
    let rows = load_rows(&options.root)?;
    let kept = SHELL_KEEP.join(", ");
    output::say(&format!(
      "final-removal: ok ({} native rows, shell kept: {kept})",
      rows.len()
    ));
    return Ok(Verdict::Clean);
  }
  for problem in &found {
    output::error(&format!("final-removal: {problem}"));
  }
  Ok(Verdict::Findings)
}

pub(crate) fn problems(root: &Path) -> Result<Vec<String>, String> {
  let mut found = Vec::new();
  for file in tracked_shell(root)? {
    found.push(format!("tracked shell file: {file}"));
  }
  if root.join(".shellcheckrc").exists() {
    found.push(".shellcheckrc remains".to_owned());
  }
  if root.join("tooling/testdata/bats").exists() {
    found.push("tooling/testdata/bats remains".to_owned());
  }
  found.extend(script_problems(root)?);
  found.extend(workflow_problems(root)?);
  found.extend(inventory_problems(root)?);
  Ok(found)
}

fn script_problems(root: &Path) -> Result<Vec<String>, String> {
  let text = read(root, "package.json")?;
  let doc: Value = serde_json::from_str(&text).map_err(|err| format!("package.json: {err}"))?;
  let scripts = doc
    .get("scripts")
    .and_then(Value::as_object)
    .ok_or_else(|| "package.json: scripts".to_owned())?;
  let mut found = Vec::new();
  for name in SHELL_SCRIPTS {
    if scripts.contains_key(name) {
      found.push(format!("shell-only package script remains: {name}"));
    }
  }
  Ok(found)
}

fn workflow_problems(root: &Path) -> Result<Vec<String>, String> {
  let workflow = read(root, ".github/workflows/tests.yml")?;
  let mut found = Vec::new();
  for job in ["shellcheck", "bats"] {
    let pattern = format!(r"(?m)^  {job}:");
    if Regex::new(&pattern)
      .map_err(|err| format!("workflow pattern: {err}"))?
      .is_match(&workflow)
    {
      found.push(format!("retired CI job remains: {job}"));
    }
  }
  if !Regex::new(r"(?m)^  typescript:")
    .map_err(|err| format!("workflow pattern: {err}"))?
    .is_match(&workflow)
  {
    found.push("replacement typescript CI job missing".to_owned());
  }
  Ok(found)
}

fn inventory_problems(root: &Path) -> Result<Vec<String>, String> {
  let rows = load_rows(root)?;
  let mut found = Vec::new();
  if rows.is_empty() {
    found.push("coverage inventory is empty".to_owned());
  }
  for row in &rows {
    found.extend(row_problems(row));
  }
  let matrix = read(root, "docs/gate-coverage-matrix.md")?;
  found.extend(matrix_problems(&matrix, rows.len())?);
  Ok(found)
}

fn row_problems(row: &Row) -> Vec<String> {
  let mut found = Vec::new();
  if row.classification != "port-native" {
    found.push(format!("{}: classification {}", row.id, row.classification));
  }
  if row.bash_required {
    found.push(format!("{}: bashRequired=true", row.id));
  }
  if !HOSTS.contains(&row.host_mechanism.as_str()) {
    found.push(format!("{}: hostMechanism {}", row.id, row.host_mechanism));
  }
  if row.implementation_status != "done" {
    found.push(format!(
      "{}: implementationStatus {}",
      row.id, row.implementation_status
    ));
  }
  found
}

fn matrix_problems(matrix: &str, rows: usize) -> Result<Vec<String>, String> {
  let expr = Regex::new(
    r"^\| `[^`]+` \| `[^`]+` \| [^|]+ \| [^|]+ \| ([^|]+) \| [^|]+ \| [^|]+ \| ([^|]+) \| ([^|]+) \|",
  )
  .map_err(|err| format!("matrix pattern: {err}"))?;
  let lines: Vec<&str> = matrix
    .lines()
    .filter(|line| line.starts_with("| `"))
    .collect();
  let mut found = Vec::new();
  if lines.len() != rows {
    found.push(format!(
      "matrix has {} rows, inventory has {rows}",
      lines.len()
    ));
  }
  for line in lines {
    found.extend(matrix_line(line, &expr));
  }
  Ok(found)
}

fn matrix_line(line: &str, expr: &Regex) -> Vec<String> {
  let Some(caps) = expr.captures(line) else {
    return vec![format!("matrix row malformed: {line}")];
  };
  let mut found = Vec::new();
  let class = caps.get(1).map_or("", |item| item.as_str().trim());
  let host = caps.get(2).map_or("", |item| item.as_str().trim());
  let bash = caps.get(3).map_or("", |item| item.as_str().trim());
  if class != "port-native" {
    found.push(format!("matrix row is not native: {line}"));
  }
  if !HOSTS.contains(&host) {
    found.push(format!("matrix row is not Bun or native: {line}"));
  }
  if bash != "no" {
    found.push(format!("matrix row requires Bash: {line}"));
  }
  found
}

fn load_rows(root: &Path) -> Result<Vec<Row>, String> {
  let text = read(root, "fixtures/gate-coverage/inventory.json")?;
  serde_json::from_str(&text).map_err(|err| format!("coverage inventory: {err}"))
}

fn tracked_shell(root: &Path) -> Result<Vec<String>, String> {
  let output = Command::new("git")
    .args(["ls-files", "-z", "*.sh", "*.bash", "*.bats"])
    .current_dir(root)
    .output()
    .map_err(|err| format!("git ls-files failed: {err}"))?;
  if !output.status.success() {
    let detail = String::from_utf8_lossy(&output.stderr);
    return Err(format!("git ls-files failed: {}", detail.trim()));
  }
  let text = String::from_utf8_lossy(&output.stdout);
  Ok(
    text
      .split('\0')
      .filter(|file| !file.is_empty() && !SHELL_KEEP.contains(file))
      .map(str::to_owned)
      .collect(),
  )
}

fn read(root: &Path, rel: &str) -> Result<String, String> {
  std::fs::read_to_string(root.join(rel)).map_err(|err| format!("cannot read {rel}: {err}"))
}

#[cfg(test)]
#[path = "tests/final_removal_test.rs"]
mod tests;
