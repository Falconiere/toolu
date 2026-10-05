//! The workspace as `cargo metadata --no-deps` reports it.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Deserialize;

/// `cargo metadata --format-version 1` output, the fields check-layers reads.
#[derive(Debug, Deserialize)]
pub struct Metadata {
  pub packages: Vec<Package>,
  pub workspace_members: Vec<String>,
  pub workspace_root: PathBuf,
}

/// One workspace package.
#[derive(Debug, Deserialize)]
pub struct Package {
  pub id: String,
  pub name: String,
  pub manifest_path: PathBuf,
  pub dependencies: Vec<Dependency>,
  pub targets: Vec<Target>,
}

/// One declared dependency of a package.
#[derive(Debug, Deserialize)]
pub struct Dependency {
  pub name: String,
  /// `None` for a normal dependency, otherwise `"dev"` or `"build"`.
  pub kind: Option<String>,
  /// Set for a path dependency.
  pub path: Option<PathBuf>,
}

/// One build target of a package.
#[derive(Debug, Deserialize)]
pub struct Target {
  pub kind: Vec<String>,
}

impl Package {
  /// The package directory.
  pub fn dir(&self) -> &Path {
    self.manifest_path.parent().unwrap_or(&self.manifest_path)
  }

  /// Whether the package has a `bin` target.
  pub fn builds_binary(&self) -> bool {
    self
      .targets
      .iter()
      .any(|target| target.kind.iter().any(|kind| kind == "bin"))
  }
}

impl Dependency {
  /// Whether the dependency is linked into the crate (normal or build), not dev-only.
  pub fn is_linked(&self) -> bool {
    self.kind.as_deref() != Some("dev")
  }
}

/// Run `cargo metadata --no-deps` for `manifest` (or the current directory).
pub fn load(manifest: Option<&Path>) -> Result<Metadata, String> {
  let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
  let mut command = Command::new(&cargo);
  command.args(["metadata", "--format-version", "1", "--no-deps"]);
  if let Some(path) = manifest {
    command.arg("--manifest-path").arg(path);
  }
  let output = command
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
