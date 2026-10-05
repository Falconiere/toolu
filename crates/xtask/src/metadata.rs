//! The workspace as `cargo metadata --no-deps` reports it.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Deserialize;

/// `cargo metadata --format-version 1` output, the fields xtask reads.
#[derive(Debug, Deserialize)]
pub(crate) struct Metadata {
  pub(crate) packages: Vec<Package>,
  pub(crate) workspace_members: Vec<String>,
  pub(crate) workspace_root: PathBuf,
}

/// One workspace package.
#[derive(Debug, Deserialize)]
pub(crate) struct Package {
  pub(crate) id: String,
  pub(crate) name: String,
  pub(crate) manifest_path: PathBuf,
  pub(crate) dependencies: Vec<Dependency>,
  pub(crate) targets: Vec<Target>,
}

/// One declared dependency of a package.
#[derive(Debug, Deserialize)]
pub(crate) struct Dependency {
  pub(crate) name: String,
  /// `None` for a normal dependency, otherwise `"dev"` or `"build"`.
  pub(crate) kind: Option<String>,
  /// Set for a path dependency.
  pub(crate) path: Option<PathBuf>,
}

/// One build target of a package.
#[derive(Debug, Deserialize)]
pub(crate) struct Target {
  pub(crate) kind: Vec<String>,
}

impl Metadata {
  /// The packages that are workspace members.
  pub(crate) fn members(&self) -> impl Iterator<Item = &Package> {
    self
      .packages
      .iter()
      .filter(|package| self.workspace_members.contains(&package.id))
  }
}

impl Package {
  /// The package directory.
  pub(crate) fn dir(&self) -> &Path {
    self.manifest_path.parent().unwrap_or(&self.manifest_path)
  }

  /// Whether the package has a target of `kind` (`bin`, `lib`, …).
  pub(crate) fn has_target(&self, kind: &str) -> bool {
    self
      .targets
      .iter()
      .any(|target| target.kind.iter().any(|each| each == kind))
  }
}

impl Dependency {
  /// Whether the dependency is linked into the crate (normal or build), not dev-only.
  pub(crate) fn is_linked(&self) -> bool {
    self.kind.as_deref() != Some("dev")
  }
}

/// Run `cargo metadata --no-deps` for the workspace at `root`.
pub(crate) fn load(root: &Path) -> Result<Metadata, String> {
  let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
  let output = Command::new(&cargo)
    .args([
      "metadata",
      "--format-version",
      "1",
      "--no-deps",
      "--manifest-path",
    ])
    .arg(root.join("Cargo.toml"))
    .output()
    .map_err(|err| format!("cannot run {} metadata: {err}", cargo.to_string_lossy()))?;
  if !output.status.success() {
    return Err(format!(
      "cargo metadata failed ({}): {}",
      output.status,
      String::from_utf8_lossy(&output.stderr).trim()
    ));
  }
  serde_json::from_slice(&output.stdout)
    .map_err(|err| format!("cargo metadata printed unreadable JSON: {err}"))
}

#[cfg(test)]
#[path = "tests/metadata_test.rs"]
mod tests;
