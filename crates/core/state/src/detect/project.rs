//! Project detection (`detect-project.ts`): `detect_project_root`, the marker probes
//! (`detect_node_pm`, `detect_rust`, `detect_python`, `detect_ts`), the linter probes
//! (`detect_ts_linter`, `detect_python_linter`, `detect_clippy`) and `to_relative_path`.
//! Every probe looks at the git toplevel of `cwd`, as the bash functions do; outside a
//! repository each answers "none". The toplevel is read from `.git`, and only
//! `detect_ts` spawns git (`ls-files`, which knows what is tracked).

use std::path::{Path, PathBuf};

use toolu_runtime::env::Env;
use toolu_runtime::git::toplevel;
use toolu_runtime::process::{Spec, run};

use super::is_regular_file;
use super::read::{Walk, each_line};

/// The package manager a lock file names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodePackageManager {
  /// `bun.lock` or `bun.lockb`.
  Bun,
  /// `pnpm-lock.yaml`.
  Pnpm,
  /// `yarn.lock`.
  Yarn,
  /// `package-lock.json`.
  Npm,
}

impl NodePackageManager {
  /// The name `detect_node_pm` prints.
  pub fn name(self) -> &'static str {
    match self {
      NodePackageManager::Bun => "bun",
      NodePackageManager::Pnpm => "pnpm",
      NodePackageManager::Yarn => "yarn",
      NodePackageManager::Npm => "npm",
    }
  }
}

/// The TypeScript linter a config file names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TsLinter {
  /// `biome.json` or `biome.jsonc`.
  Biome,
  /// `.oxlintrc.json`.
  Oxc,
  /// `.eslintrc*` or `eslint.config.*`.
  Eslint,
}

impl TsLinter {
  /// The name `detect_ts_linter` prints.
  pub fn name(self) -> &'static str {
    match self {
      TsLinter::Biome => "biome",
      TsLinter::Oxc => "oxc",
      TsLinter::Eslint => "eslint",
    }
  }
}

/// The Python linter a config names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PythonLinter {
  /// `ruff.toml`, `.ruff.toml` or a `[tool.ruff` line in `pyproject.toml`.
  Ruff,
}

impl PythonLinter {
  /// The name `detect_python_linter` prints.
  pub fn name(self) -> &'static str {
    match self {
      PythonLinter::Ruff => "ruff",
    }
  }
}

/// The lock files in bash's order.
const LOCK_FILES: [(&str, NodePackageManager); 5] = [
  ("bun.lock", NodePackageManager::Bun),
  ("bun.lockb", NodePackageManager::Bun),
  ("pnpm-lock.yaml", NodePackageManager::Pnpm),
  ("yarn.lock", NodePackageManager::Yarn),
  ("package-lock.json", NodePackageManager::Npm),
];

/// `detect_project_root`: the git toplevel of `cwd`, or `None` outside a repository.
pub fn project_toplevel(env: &Env, cwd: &Path) -> Option<PathBuf> {
  toplevel(env, cwd)
}

/// `detect_project_name`: the toplevel's basename.
pub fn project_name(env: &Env, cwd: &Path) -> Option<String> {
  let root = project_toplevel(env, cwd)?;
  let name = root.file_name().map(|name| name.to_string_lossy());
  Some(name.unwrap_or_default().into_owned())
}

/// Whether any of `names` is a regular file under `root`.
fn has_any(root: &Path, names: &[&str]) -> bool {
  names.iter().any(|name| is_regular_file(&root.join(name)))
}

/// `detect_node_pm`: the first lock file present, in bash's order.
pub fn node_package_manager(env: &Env, cwd: &Path) -> Option<NodePackageManager> {
  let root = project_toplevel(env, cwd)?;
  let lock = LOCK_FILES
    .iter()
    .find(|(file, _)| is_regular_file(&root.join(file)));
  lock.map(|(_, manager)| *manager)
}

/// `detect_rust`: a `Cargo.toml` at the root.
pub fn detect_rust(env: &Env, cwd: &Path) -> bool {
  project_toplevel(env, cwd).is_some_and(|root| has_any(&root, &["Cargo.toml"]))
}

/// `detect_python`: any of the four Python markers at the root.
pub fn detect_python(env: &Env, cwd: &Path) -> bool {
  let markers = [
    "pyproject.toml",
    "setup.py",
    "setup.cfg",
    "requirements.txt",
  ];
  project_toplevel(env, cwd).is_some_and(|root| has_any(&root, &markers))
}

/// `detect_ts`: git tracks a `tsconfig*.json`. The same two pathspecs go to
/// `git ls-files`, so git's own pathspec rules (a `*` also matches `/`) decide.
pub fn detect_ts(env: &Env, cwd: &Path) -> bool {
  let Some(root) = project_toplevel(env, cwd) else {
    return false;
  };
  let mut spec = Spec::new(["git", "-C"]);
  spec.argv.push(root.display().to_string());
  spec
    .argv
    .extend(["ls-files", "**/tsconfig*.json", "tsconfig*.json"].map(String::from));
  spec.env = Some(env.clone());
  run(&spec).is_ok_and(|out| out.stdout.chars().any(|c| c != '\n'))
}

/// `compgen -G`: whether any entry of `root`, file or directory, is named like an
/// eslint config.
fn has_eslint_config(root: &Path) -> bool {
  std::fs::read_dir(root).is_ok_and(|entries| {
    entries.flatten().any(|entry| {
      let name = entry.file_name();
      let name = name.as_encoded_bytes();
      name.starts_with(b".eslintrc") || name.starts_with(b"eslint.config.")
    })
  })
}

/// `detect_ts_linter`: biome, then oxc, then eslint, by config file at the root.
pub fn ts_linter(env: &Env, cwd: &Path) -> Option<TsLinter> {
  let root = project_toplevel(env, cwd)?;
  if has_any(&root, &["biome.json", "biome.jsonc"]) {
    return Some(TsLinter::Biome);
  }
  if has_any(&root, &[".oxlintrc.json"]) {
    return Some(TsLinter::Oxc);
  }
  has_eslint_config(&root).then_some(TsLinter::Eslint)
}

/// `detect_python_linter`: a ruff config file, or a `[tool.ruff` line in `pyproject.toml`.
pub fn python_linter(env: &Env, cwd: &Path) -> Option<PythonLinter> {
  let root = project_toplevel(env, cwd)?;
  let pyproject = root.join("pyproject.toml");
  let ruff = has_any(&root, &["ruff.toml", ".ruff.toml"])
    || (is_regular_file(&pyproject)
      && each_line(&pyproject, |line| line.starts_with(b"[tool.ruff")) == Walk::Stopped);
  ruff.then_some(PythonLinter::Ruff)
}

/// `detect_clippy`: a clippy config at the root.
pub fn detect_clippy(env: &Env, cwd: &Path) -> bool {
  project_toplevel(env, cwd).is_some_and(|root| has_any(&root, &["clippy.toml", ".clippy.toml"]))
}

/// `to_relative_path`: `path` relative to the toplevel when under it, else unchanged.
pub fn to_relative_path(path: &str, env: &Env, cwd: &Path) -> String {
  if path.is_empty() {
    return String::new();
  }
  let root = project_toplevel(env, cwd);
  let prefix = root
    .as_deref()
    .and_then(Path::to_str)
    .map(|root| format!("{root}/"));
  let relative = prefix.and_then(|prefix| path.strip_prefix(prefix.as_str()));
  relative.unwrap_or(path).to_owned()
}

#[cfg(test)]
#[path = "tests/project_test.rs"]
mod tests;
