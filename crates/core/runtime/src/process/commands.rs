//! The fixed commands other modules run: `git rev-parse --show-toplevel` for
//! the project root and `codex plugin list --json` for the Codex plugins.

use std::path::{Path, PathBuf};

use super::{Spec, run};
use crate::env::Env;

/// `git rev-parse --show-toplevel` in `cwd` with exactly `env`, or `None` when
/// git is missing, fails or prints nothing.
pub fn git_toplevel(env: &Env, cwd: &Path) -> Option<PathBuf> {
  let mut spec = Spec::new(["git", "rev-parse", "--show-toplevel"]);
  spec.cwd = Some(cwd.to_path_buf());
  spec.env = Some(env.clone());
  let output = run(&spec).ok().filter(|output| output.exit_code == 0)?;
  let top = output.stdout.trim();
  (!top.is_empty()).then(|| PathBuf::from(top))
}

/// The stdout of `codex plugin list --json` run with exactly `env`, or `None`
/// when the CLI is missing or exits non-zero.
pub fn codex_plugin_list(env: &Env) -> Option<String> {
  let mut spec = Spec::new(["codex", "plugin", "list", "--json"]);
  spec.env = Some(env.clone());
  let output = run(&spec).ok().filter(|output| output.exit_code == 0)?;
  Some(output.stdout)
}

#[cfg(test)]
#[path = "tests/commands_test.rs"]
mod tests;
