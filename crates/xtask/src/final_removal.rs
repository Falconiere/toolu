//! `cargo xtask final-removal`: the TypeScript and Bun removal holds (#440).
//! No TypeScript source but benchmark fixtures, no Bun lockfile, bundle or
//! source tree, no shell but the installer, no `bun` in a hook or a workflow,
//! no JavaScript workspace, and a coverage inventory that is native throughout.

use std::path::Path;
use std::process::Command;

use regex::Regex;
use serde::Deserialize;

use crate::options::Options;
use crate::{Verdict, output};

/// The one shell file that stays: the curl installer.
const SHELL_KEEP: &str = "install.sh";

/// Where TypeScript may remain: input files of the deterministic benchmark.
const TS_KEEP: &str = "benchmarks/fixtures/";

/// Trees the removal deleted.
const GONE: [&str; 3] = [
  "packages",
  "tools/toolu-conformance",
  "tooling/testdata/bats",
];

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Row {
  id: String,
  classification: String,
  host_mechanism: String,
  implementation_status: String,
  bash_required: bool,
}

/// Judge the tree, the hooks, the workflows and the native coverage inventory.
pub(crate) fn run(options: &Options) -> Result<Verdict, String> {
  let found = problems(&options.root)?;
  if found.is_empty() {
    let rows = load_rows(&options.root)?;
    output::say(&format!(
      "final-removal: ok ({} native rows, shell kept: {SHELL_KEEP})",
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
  let mut found = tracked_problems(&tracked(root)?);
  for dir in GONE {
    if root.join(dir).exists() {
      found.push(format!("{dir} remains"));
    }
  }
  if root.join(".shellcheckrc").exists() {
    found.push(".shellcheckrc remains".to_owned());
  }
  found.extend(plugin_problems(root)?);
  found.extend(package_problems(root)?);
  found.extend(workflow_problems(root)?);
  found.extend(inventory_problems(root)?);
  Ok(found)
}

/// TypeScript, Bun lockfiles and shell among the tracked files.
pub(crate) fn tracked_problems(files: &[String]) -> Vec<String> {
  let mut found = Vec::new();
  for file in files {
    let name = file.rsplit('/').next().unwrap_or(file);
    let ext = Path::new(name).extension().and_then(|ext| ext.to_str());
    let script = matches!(ext, Some("ts" | "tsx" | "mts" | "cts"));
    if script && !file.starts_with(TS_KEEP) {
      found.push(format!("tracked TypeScript file: {file}"));
    }
    if matches!(name, "bun.lock" | "bun.lockb" | "bunfig.toml") {
      found.push(format!("tracked Bun file: {file}"));
    }
    if matches!(ext, Some("sh" | "bash" | "bats")) && file != SHELL_KEEP {
      found.push(format!("tracked shell file: {file}"));
    }
  }
  found
}

/// Bundles, hook sources, and `bun` in a plugin's `hooks.json`.
fn plugin_problems(root: &Path) -> Result<Vec<String>, String> {
  let bun = word_bun()?;
  let mut found = Vec::new();
  let Ok(plugins) = std::fs::read_dir(root.join("plugins")) else {
    return Ok(found);
  };
  for plugin in plugins.flatten() {
    let name = plugin.file_name().to_string_lossy().into_owned();
    for tree in ["hooks/dist", "hooks/src"] {
      if plugin.path().join(tree).exists() {
        found.push(format!("plugins/{name}/{tree} remains"));
      }
    }
    let hooks = plugin.path().join("hooks/hooks.json");
    let text = read_or_empty(&hooks)?;
    if bun.is_match(&text) || text.contains("TOOLU_RUNTIME") || text.contains("hooks/dist") {
      found.push(format!("plugins/{name}/hooks/hooks.json still names Bun"));
    }
  }
  Ok(found)
}

/// The root `package.json` is the release anchor and jscpd's manifest, not a workspace.
fn package_problems(root: &Path) -> Result<Vec<String>, String> {
  let text = read(root, "package.json")?;
  let doc: serde_json::Value =
    serde_json::from_str(&text).map_err(|err| format!("package.json: {err}"))?;
  Ok(if doc.get("workspaces").is_some() {
    vec!["package.json still declares workspaces".to_owned()]
  } else {
    Vec::new()
  })
}

/// No workflow installs or runs Bun; the retired jobs stay gone, `typescript` stays.
fn workflow_problems(root: &Path) -> Result<Vec<String>, String> {
  let bun = word_bun()?;
  let mut found = Vec::new();
  let dir = root.join(".github/workflows");
  let entries =
    std::fs::read_dir(&dir).map_err(|err| format!("cannot read {}: {err}", dir.display()))?;
  for entry in entries {
    let entry = entry.map_err(|err| format!("cannot read {}: {err}", dir.display()))?;
    let name = entry.file_name().to_string_lossy().into_owned();
    let text = read_or_empty(&entry.path())?;
    if bun.is_match(&text) || text.contains("setup-bun") {
      found.push(format!("workflow {name} still uses Bun"));
    }
  }
  let tests = read(root, ".github/workflows/tests.yml")?;
  for job in ["shellcheck", "bats", "rust-conformance"] {
    if job_pattern(job)?.is_match(&tests) {
      found.push(format!("retired CI job remains: {job}"));
    }
  }
  if !job_pattern("typescript")?.is_match(&tests) {
    found.push("replacement typescript CI job missing".to_owned());
  }
  Ok(found)
}

fn job_pattern(job: &str) -> Result<Regex, String> {
  Regex::new(&format!(r"(?m)^  {job}:")).map_err(|err| format!("workflow pattern: {err}"))
}

/// `bun` as a word, lower case, as a command names it.
fn word_bun() -> Result<Regex, String> {
  Regex::new(r"\bbun\b").map_err(|err| format!("bun pattern: {err}"))
}

/// Every file `git ls-files` lists under `root`.
fn tracked(root: &Path) -> Result<Vec<String>, String> {
  let output = Command::new("git")
    .args(["ls-files", "-z"])
    .current_dir(root)
    .output()
    .map_err(|err| format!("git ls-files failed: {err}"))?;
  if !output.status.success() {
    let detail = String::from_utf8_lossy(&output.stderr);
    return Err(format!("git ls-files failed: {}", detail.trim()));
  }
  Ok(
    String::from_utf8_lossy(&output.stdout)
      .split('\0')
      .filter(|file| !file.is_empty())
      .map(str::to_owned)
      .collect(),
  )
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
  if row.host_mechanism != "native" {
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
  if host != "native" {
    found.push(format!("matrix row is not native: {line}"));
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

/// A file's text, or empty when it does not exist; any other error is reported.
fn read_or_empty(path: &Path) -> Result<String, String> {
  match std::fs::read_to_string(path) {
    Ok(text) => Ok(text),
    Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
    Err(err) => Err(format!("cannot read {}: {err}", path.display())),
  }
}

fn read(root: &Path, rel: &str) -> Result<String, String> {
  std::fs::read_to_string(root.join(rel)).map_err(|err| format!("cannot read {rel}: {err}"))
}

#[cfg(test)]
#[path = "tests/final_removal_test.rs"]
mod tests;
