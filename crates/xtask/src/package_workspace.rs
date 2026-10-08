//! `cargo xtask check-workspace`: package presence, Bun 1.4, and export smokes.

use std::path::Path;
use std::process::Command;

use serde_json::Value;

use crate::ci_yaml::bun_binary;
use crate::options::Options;
use crate::{Verdict, output};

const PACKAGES: [&str; 5] = [
  "packages/toolu-core",
  "tools/toolu-opencode",
  "tools/toolu-conformance",
  "tools/toolu-cli",
  "tooling",
];

const WORKSPACES: [&str; 4] = [
  "packages/toolu-core",
  "tools/toolu-opencode",
  "tools/toolu-conformance",
  "tools/toolu-cli",
];

const SMOKE: &str = r#"
const { parseDecision } = await import("@toolu/core/decision");
if (parseDecision({ kind: "allow" }).kind !== "allow") throw new Error("decision");
const { detectHost } = await import("@toolu/core/host");
if (detectHost({ env: { PLUGIN_ROOT: "/p" } }) !== "codex") throw new Error("host");
const { classifyStub } = await import("@toolu/opencode/plugin-stub");
if (classifyStub("shell-out") !== "shell-out") throw new Error("classify");
const plugin = (await import("@toolu/opencode")).default;
if (plugin.id !== "toolu") throw new Error("default entry");
const { runProtectedFilesConformance } = await import("@toolu/conformance/run-stub");
const conformance = await runProtectedFilesConformance();
if (!conformance.pass) throw new Error(conformance.message ?? "conformance");
"#;

/// Smoke the Bun workspace under `--root`.
pub(crate) fn run(options: &Options) -> Result<Verdict, String> {
  Ok(report(problem(&options.root)?))
}

pub(crate) fn problem(root: &Path) -> Result<Option<String>, String> {
  let bun = bun_binary()?;
  let version = bun_version(&bun)?;
  if !version.starts_with("1.4.") {
    return Ok(Some(format!(
      "Bun {version} not in supported 1.4.x range (docs/portable-core.md)"
    )));
  }
  for pkg in PACKAGES {
    if !root.join(pkg).join("package.json").is_file() {
      return Ok(Some(format!("missing {pkg}/package.json")));
    }
  }
  if let Some(message) = workspaces(root)? {
    return Ok(Some(message));
  }
  if let Some(message) = export_smokes(root, &bun)? {
    return Ok(Some(message));
  }
  cli_version(root, &bun)
}

fn workspaces(root: &Path) -> Result<Option<String>, String> {
  let doc = read_json(root, "package.json")?;
  let listed = doc
    .get("workspaces")
    .and_then(Value::as_array)
    .ok_or_else(|| "package.json has an unexpected shape: workspaces".to_owned())?;
  let names: Vec<&str> = listed.iter().filter_map(Value::as_str).collect();
  if WORKSPACES.iter().all(|ws| names.contains(ws)) {
    return Ok(None);
  }
  Ok(Some(
    "package.json workspaces must include tooling, packages/*, tools/*".to_owned(),
  ))
}

fn export_smokes(root: &Path, bun: &Path) -> Result<Option<String>, String> {
  let output = Command::new(bun)
    .arg("-e")
    .arg(SMOKE)
    .current_dir(root)
    .output()
    .map_err(|err| format!("cannot run bun: {err}"))?;
  if output.status.success() {
    return Ok(None);
  }
  Ok(Some(smoke_message(&output.stderr)))
}

fn cli_version(root: &Path, bun: &Path) -> Result<Option<String>, String> {
  let output = Command::new(bun)
    .args(["run", "tools/toolu-cli/src/cli.ts", "--version"])
    .current_dir(root)
    .output()
    .map_err(|err| format!("cannot run bun: {err}"))?;
  if !output.status.success() {
    let detail = String::from_utf8_lossy(&output.stderr);
    return Ok(Some(format!(
      "toolu-cli --version failed: {}",
      detail.trim()
    )));
  }
  let cli = String::from_utf8_lossy(&output.stdout).trim().to_owned();
  let expected = read_json(root, "tools/toolu-cli/package.json")?;
  let version = expected
    .get("version")
    .and_then(Value::as_str)
    .ok_or_else(|| "tools/toolu-cli/package.json has an unexpected shape: version".to_owned())?;
  if cli == version {
    return Ok(None);
  }
  Ok(Some(format!("toolu-cli --version reported '{cli}'")))
}

fn bun_version(bun: &Path) -> Result<String, String> {
  let output = Command::new(bun)
    .arg("--version")
    .output()
    .map_err(|err| format!("cannot run bun: {err}"))?;
  if !output.status.success() {
    return Err("bun is not installed".to_owned());
  }
  Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn read_json(root: &Path, rel: &str) -> Result<Value, String> {
  let text = std::fs::read_to_string(root.join(rel))
    .map_err(|err| format!("{rel} is not readable JSON: {err}"))?;
  serde_json::from_str(&text).map_err(|err| format!("{rel} is not readable JSON: {err}"))
}

fn smoke_message(stderr: &[u8]) -> String {
  let text = String::from_utf8_lossy(stderr);
  for token in ["decision", "host", "classify", "default entry"] {
    if text.contains(token) {
      return token.to_owned();
    }
  }
  text
    .lines()
    .find(|line| !line.trim().is_empty())
    .unwrap_or("export smoke failed")
    .to_owned()
}

fn report(problem: Option<String>) -> Verdict {
  if let Some(message) = problem {
    output::error(&format!("check-workspace: {message}"));
    return Verdict::Findings;
  }
  output::say("check-workspace: ok");
  Verdict::Clean
}

#[cfg(test)]
#[path = "tests/package_workspace_test.rs"]
mod tests;
