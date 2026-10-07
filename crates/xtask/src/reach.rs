//! `cargo xtask check-reach`: every Rust file lies inside a workspace member,
//! so no code escapes the gate by sitting outside the workspace. The one
//! exception is a crate under `vendor/` that the root manifest excludes from
//! the workspace: third-party code cargo builds only as a path dependency
//! (`vendor/tree-sitter-bash`, #416).

use std::path::{Path, PathBuf};

use crate::options::Options;
use crate::workspace::Workspace;
use crate::{Verdict, output};

/// Listed `.rs` files outside every member directory and every vendored crate.
pub(crate) fn run(options: &Options) -> Result<Verdict, String> {
  let workspace = Workspace::load(&options.root)?;
  let vendored = vendored(&options.root)?;
  let outside: Vec<String> = unreached(&workspace, &vendored);
  Ok(output::findings("check-reach", &outside))
}

/// The directories under `vendor/` that `[workspace] exclude` names in the
/// manifest at `root`; any other excluded directory is still judged.
pub(crate) fn vendored(root: &Path) -> Result<Vec<PathBuf>, String> {
  let path = root.join("Cargo.toml");
  let text = std::fs::read_to_string(&path)
    .map_err(|err| format!("cannot read {}: {err}", path.display()))?;
  let manifest: toml::Table = text
    .parse()
    .map_err(|err| format!("cannot parse {}: {err}", path.display()))?;
  let excluded = manifest
    .get("workspace")
    .and_then(|workspace| workspace.get("exclude"))
    .and_then(toml::Value::as_array);
  Ok(
    excluded
      .into_iter()
      .flatten()
      .filter_map(toml::Value::as_str)
      .map(PathBuf::from)
      .filter(|dir| dir.starts_with("vendor") && dir.components().count() > 1)
      .collect(),
  )
}

/// One line per `.rs` file no member holds, outside the `vendored` crates.
pub(crate) fn unreached(workspace: &Workspace, vendored: &[PathBuf]) -> Vec<String> {
  workspace
    .files
    .iter()
    .filter(|file| file.extension().is_some_and(|ext| ext == "rs"))
    .filter(|file| workspace.member_of(file).is_none())
    .filter(|file| !vendored.iter().any(|dir| file.starts_with(dir)))
    .map(|file| {
      format!(
        "reach {}: outside every workspace member",
        file.to_string_lossy().replace('\\', "/")
      )
    })
    .collect()
}

#[cfg(test)]
#[path = "tests/reach_test.rs"]
mod tests;
