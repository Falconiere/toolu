//! `npm pack --dry-run --json`, and the temp copy `@toolu/opencode` packs from.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use serde::Deserialize;

/// One file `npm pack` would publish, with its mode.
#[derive(Debug, Deserialize)]
pub(crate) struct PackedFile {
  pub(crate) path: String,
  pub(crate) mode: u32,
}

#[derive(Deserialize)]
struct Report {
  files: Vec<PackedFile>,
}

/// A temp repository layout. Dropping it removes the copy.
pub(crate) struct Stage {
  path: PathBuf,
  pub(crate) package: PathBuf,
}

impl Stage {
  /// Copy `tools/toolu-opencode` and link the real `plugins/` catalog beside it.
  pub(crate) fn opencode(root: &Path) -> Result<Self, String> {
    static TICK: AtomicU64 = AtomicU64::new(0);
    let tick = TICK.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
      "pack-inventory-opencode-{}-{tick}",
      std::process::id()
    ));
    std::fs::create_dir_all(&path).map_err(|err| format!("pack: cannot create stage: {err}"))?;
    let package = path.join("repo/tools/toolu-opencode");
    copy_package(&root.join("tools/toolu-opencode"), &package)?;
    let plugins = path.join("repo/plugins");
    std::os::unix::fs::symlink(root.join("plugins"), &plugins)
      .map_err(|err| format!("pack: cannot link plugins: {err}"))?;
    Ok(Self { path, package })
  }
}

impl Drop for Stage {
  fn drop(&mut self) {
    let _ = std::fs::remove_dir_all(&self.path);
  }
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

fn copy_package(source: &Path, dest: &Path) -> Result<(), String> {
  std::fs::create_dir_all(dest)
    .map_err(|err| format!("pack: cannot create {}: {err}", dest.display()))?;
  for entry in std::fs::read_dir(source)
    .map_err(|err| format!("pack: cannot list {}: {err}", source.display()))?
  {
    let entry = entry.map_err(|err| format!("pack: cannot list {}: {err}", source.display()))?;
    let name = entry.file_name();
    if name == "plugins" || name == "node_modules" {
      continue;
    }
    copy_any(&entry.path(), &dest.join(&name))?;
  }
  Ok(())
}

fn copy_any(source: &Path, dest: &Path) -> Result<(), String> {
  let meta = std::fs::metadata(source)
    .map_err(|err| format!("pack: cannot read {}: {err}", source.display()))?;
  if meta.is_dir() {
    std::fs::create_dir_all(dest)
      .map_err(|err| format!("pack: cannot create {}: {err}", dest.display()))?;
    for entry in std::fs::read_dir(source)
      .map_err(|err| format!("pack: cannot list {}: {err}", source.display()))?
    {
      let entry = entry.map_err(|err| format!("pack: cannot list {}: {err}", source.display()))?;
      copy_any(&entry.path(), &dest.join(entry.file_name()))?;
    }
    return Ok(());
  }
  if meta.is_file() {
    if let Some(parent) = dest.parent() {
      std::fs::create_dir_all(parent)
        .map_err(|err| format!("pack: cannot create {}: {err}", parent.display()))?;
    }
    std::fs::copy(source, dest)
      .map_err(|err| format!("pack: cannot copy {}: {err}", source.display()))?;
  }
  Ok(())
}

#[cfg(test)]
#[path = "tests/pack_npm_test.rs"]
mod tests;
