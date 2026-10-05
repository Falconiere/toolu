//! `cargo xtask guardrails`: the rules of the quality bar that clippy, rustc,
//! rustfmt and cargo-deny cannot see (#455). Data: `tooling/conventions/guardrails/rust/`.

mod attributes;
mod capabilities;
mod colocated;
mod hygiene;
mod inventory;
mod leftovers;
mod public_api;
mod secrets;
mod size;
mod structure;
mod syntax;
mod test_layout;
mod thresholds;

use std::fmt;
use std::path::Path;

use crate::Verdict;
use crate::data::{self, Folders, Inventory, Limits, Rules};
use crate::options::Options;
use crate::output;
use crate::source::Source;
use crate::workspace::Workspace;

/// One violation: rule id, file, 1-based line and what is wrong.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Finding {
  pub(crate) path: String,
  pub(crate) line: usize,
  pub(crate) rule: &'static str,
  pub(crate) message: String,
}

impl Finding {
  /// A finding at `line` of `path`.
  pub(crate) fn new(rule: &'static str, path: &str, line: usize, message: String) -> Self {
    Finding {
      path: path.to_owned(),
      line,
      rule,
      message,
    }
  }
}

impl fmt::Display for Finding {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(
      f,
      "{} {}:{}: {}",
      self.rule, self.path, self.line, self.message
    )
  }
}

/// Everything the rules read.
pub(crate) struct Context<'w> {
  pub(crate) workspace: &'w Workspace,
  pub(crate) rules: Rules,
  pub(crate) folders: Folders,
  pub(crate) inventory: Inventory,
  pub(crate) limits: Limits,
  /// Every listed `.rs` file under `crates/`.
  pub(crate) sources: Vec<Source<'w>>,
}

impl<'w> Context<'w> {
  /// Load the data and every Rust source of `workspace`.
  pub(crate) fn load(workspace: &'w Workspace) -> Result<Self, String> {
    let root = workspace.root.as_path();
    let sources = workspace
      .files_in("crates", "rs")
      .map(|rel| Source::load(workspace, rel))
      .collect::<Result<Vec<_>, _>>()?;
    Ok(Context {
      workspace,
      rules: data::rules(root)?,
      folders: data::load(root, "folders.json")?,
      inventory: data::load(root, "inventory.json")?,
      limits: data::limits(root)?,
      sources,
    })
  }

  /// The source at `rel`, if it is a listed Rust file.
  pub(crate) fn source(&self, rel: &Path) -> Option<&Source<'w>> {
    self.sources.iter().find(|source| source.rel == rel)
  }
}

/// Run every rule over the workspace at `options.root`.
pub(crate) fn run(options: &Options) -> Result<Verdict, String> {
  let workspace = Workspace::load(&options.root)?;
  let ctx = Context::load(&workspace)?;
  let found = check(&ctx)?;
  Ok(output::findings("guardrails", &found))
}

/// Every finding, sorted by file and line.
pub(crate) fn check(ctx: &Context<'_>) -> Result<Vec<Finding>, String> {
  let mut found = syntax::check(ctx);
  found.extend(size::check(ctx));
  found.extend(test_layout::check(ctx));
  found.extend(colocated::check(ctx));
  found.extend(inventory::check(ctx)?);
  found.extend(attributes::check(ctx));
  found.extend(structure::check(ctx));
  found.extend(hygiene::check(ctx)?);
  found.extend(capabilities::check(ctx));
  found.extend(public_api::check(ctx));
  found.extend(leftovers::check(ctx)?);
  found.extend(secrets::check(ctx)?);
  found.extend(thresholds::check(ctx)?);
  found.sort();
  Ok(found)
}

#[cfg(test)]
#[path = "tests/guardrails_test.rs"]
mod tests;
