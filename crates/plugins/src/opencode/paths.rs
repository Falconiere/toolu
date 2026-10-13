//! Every file the `OpenCode` verbs read or write for one project, the way the
//! pinned host and `@toolu/opencode` resolve them.

use std::path::{Path, PathBuf};

use toolu_runtime::env::Env;
use toolu_runtime::git;

/// Where an `OpenCode` edit lands: `--scope user` is the global config.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Scope {
  /// `${XDG_CONFIG_HOME:-~/.config}/opencode`.
  Global,
  /// The git worktree root.
  Project,
}

impl Scope {
  /// How notes name it.
  pub(crate) fn name(self) -> &'static str {
    match self {
      Scope::Global => "global",
      Scope::Project => "project",
    }
  }
}

/// The files of one project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Paths {
  /// Global config files, ascending priority: the last defining `plugin` wins.
  pub(crate) global_files: Vec<PathBuf>,
  /// Worktree-root config files in the host's load order.
  pub(crate) project_files: Vec<PathBuf>,
  /// The global selection file.
  pub(crate) selection_global: PathBuf,
  /// The project selection file.
  pub(crate) selection_project: PathBuf,
  /// The project's `.opencode/toolu.config.json`.
  pub(crate) toolu_config: PathBuf,
}

impl Paths {
  /// The selection file of `scope`.
  pub(crate) fn selection(&self, scope: Scope) -> &Path {
    match scope {
      Scope::Global => &self.selection_global,
      Scope::Project => &self.selection_project,
    }
  }
}

fn absolute(cwd: &Path, path: &str) -> PathBuf {
  cwd.join(path)
}

/// `OpenCode`'s own config directory; toolu's overrides never move it.
fn host_config_dir(env: &Env, cwd: &Path) -> PathBuf {
  let base = env
    .get("XDG_CONFIG_HOME")
    .map_or_else(|| env.home().join(".config"), |dir| absolute(cwd, dir));
  base.join("opencode")
}

/// The selection root `@toolu/opencode` reads: `TOOLU_CONFIG_DIR` (unless an
/// enclosing `OpenCode` session set it to its data root), `TOOLU_OPENCODE_HOME`,
/// else the host's config directory.
fn selection_root(env: &Env, cwd: &Path) -> PathBuf {
  let config_dir = env.get("TOOLU_CONFIG_DIR");
  let inherited = config_dir.is_some() && config_dir == env.get("TOOLU_OPENCODE_DATA_ROOT");
  let chosen = config_dir
    .filter(|_| !inherited)
    .or_else(|| env.get("TOOLU_OPENCODE_HOME"));
  chosen.map_or_else(|| host_config_dir(env, cwd), |dir| absolute(cwd, dir))
}

/// The git worktree containing `cwd`, else `cwd`.
pub(crate) fn project_root_of(cwd: &Path, env: &Env) -> PathBuf {
  git::toplevel(env, cwd).unwrap_or_else(|| cwd.to_path_buf())
}

/// The files of the project at `root`.
pub(crate) fn paths(root: &Path, env: &Env, cwd: &Path) -> Paths {
  let global = host_config_dir(env, cwd);
  let dot = root.join(".opencode");
  Paths {
    global_files: ["config.json", "opencode.json", "opencode.jsonc"]
      .iter()
      .map(|name| global.join(name))
      .collect(),
    project_files: vec![
      root.join("opencode.json"),
      root.join("opencode.jsonc"),
      dot.join("opencode.json"),
      dot.join("opencode.jsonc"),
    ],
    selection_global: selection_root(env, cwd).join("toolu").join("plugins.json"),
    selection_project: dot.join("toolu").join("plugins.json"),
    toolu_config: dot.join("toolu.config.json"),
  }
}

#[cfg(test)]
#[path = "tests/paths_test.rs"]
mod tests;
