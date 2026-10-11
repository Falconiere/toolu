//! `cargo xtask check-hooks`: every native `hooks.json` entry equals the launcher
//! generator's output and declares a `timeout`, every Bun bundle entry equals
//! the `@toolu/core/launcher` command pair, and every `plugin.json` declares
//! the binary's `hookProtocol` (#412).

use std::fmt;
use std::path::Path;

use crate::options::Options;
use crate::{Verdict, hooks_entries, hooks_manifests, output};

/// One problem in a plugin's hook wiring.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Finding {
  /// Repository-relative file.
  pub(crate) file: String,
  /// Where in it (`<Event>[i].hooks[j]`), or empty for the whole file.
  pub(crate) at: String,
  /// What is wrong.
  pub(crate) problem: String,
  /// The string the entry should hold, when there is one.
  pub(crate) expected: Option<String>,
}

impl fmt::Display for Finding {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "check-hooks: {}", self.file)?;
    if !self.at.is_empty() {
      write!(f, ": {}", self.at)?;
    }
    write!(f, ": {}", self.problem)?;
    if let Some(expected) = &self.expected {
      write!(f, "\n  expected: {expected}")?;
    }
    Ok(())
  }
}

/// Run the check on the repository at `options.root`.
pub(crate) fn run(options: &Options) -> Result<Verdict, String> {
  let found = check(&options.root)?;
  Ok(output::findings("check-hooks", &found))
}

/// Every finding under `<root>/plugins`, in plugin order.
pub(crate) fn check(root: &Path) -> Result<Vec<Finding>, String> {
  let mut plugins: Vec<String> = std::fs::read_dir(root.join("plugins"))
    .map_err(|err| format!("cannot list {}/plugins: {err}", root.display()))?
    .filter_map(Result::ok)
    .filter(|entry| entry.path().is_dir())
    .map(|entry| entry.file_name().to_string_lossy().into_owned())
    .collect();
  plugins.sort();
  let mut found = Vec::new();
  for plugin in &plugins {
    found.extend(hooks_entries::check(root, plugin));
    found.extend(hooks_manifests::check(root, plugin));
  }
  Ok(found)
}

#[cfg(test)]
#[path = "tests/check_hooks_test.rs"]
mod tests;
