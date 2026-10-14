//! `npm pack --dry-run --json`.

use std::path::Path;
use std::process::{Command, Stdio};

use serde::Deserialize;

/// One file `npm pack` would publish.
#[derive(Debug, Deserialize)]
pub(crate) struct PackedFile {
  pub(crate) path: String,
}

#[derive(Deserialize)]
struct Report {
  files: Vec<PackedFile>,
}

/// The files `npm publish` would ship from `directory`.
pub(crate) fn packed_files(directory: &Path) -> Result<Vec<PackedFile>, String> {
  let output = Command::new("npm")
    .args(["pack", "--json", "--dry-run"])
    .current_dir(directory)
    .stdin(Stdio::null())
    .output()
    .map_err(|err| format!("pack: npm pack in {}: {err}", directory.display()))?;
  if !output.status.success() {
    let code = output
      .status
      .code()
      .map_or_else(|| "signal".to_owned(), |code| code.to_string());
    let stderr = String::from_utf8_lossy(&output.stderr);
    return Err(format!(
      "pack: npm pack in {} failed (exit {code}): {}",
      directory.display(),
      stderr.trim()
    ));
  }
  let stdout = String::from_utf8_lossy(&output.stdout);
  parse_report(&stdout)
}

fn parse_report(stdout: &str) -> Result<Vec<PackedFile>, String> {
  let report = stdout
    .lines()
    .skip_while(|line| !line.starts_with('['))
    .collect::<Vec<_>>()
    .join("\n");
  if report.is_empty() {
    let preview: String = stdout.chars().take(500).collect();
    return Err(format!("pack: npm pack printed no JSON report: {preview}"));
  }
  let parsed: Vec<Report> =
    serde_json::from_str(&report).map_err(|err| format!("pack: npm pack JSON: {err}"))?;
  parsed
    .into_iter()
    .next()
    .map(|item| item.files)
    .ok_or_else(|| "pack: npm pack reported no package".to_owned())
}

#[cfg(test)]
#[path = "tests/pack_npm_test.rs"]
mod tests;
