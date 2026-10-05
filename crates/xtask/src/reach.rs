//! `cargo xtask check-reach`: every Rust file lies inside a workspace member,
//! so no code escapes the gate by sitting outside the workspace.

use crate::options::Options;
use crate::workspace::Workspace;
use crate::{Verdict, output};

/// Listed `.rs` files outside every member directory.
pub(crate) fn run(options: &Options) -> Result<Verdict, String> {
  let workspace = Workspace::load(&options.root)?;
  let outside: Vec<String> = unreached(&workspace);
  Ok(output::findings("check-reach", &outside))
}

/// One line per `.rs` file no member holds.
pub(crate) fn unreached(workspace: &Workspace) -> Vec<String> {
  workspace
    .files
    .iter()
    .filter(|file| file.extension().is_some_and(|ext| ext == "rs"))
    .filter(|file| workspace.member_of(file).is_none())
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
