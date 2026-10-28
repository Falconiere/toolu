//! The committed golden set on disk: `expectations.json` plus, per case, a
//! `files/base/` and `files/tip/` tree and an optional `memory.json`. No `.git`
//! directory is committed; `materialize_repo` builds a real two-commit repository
//! from the trees so the production `shape` stage can diff it.

use std::path::{Path, PathBuf};

use tempfile::TempDir;
use toolu_runtime::env::Env;
use toolu_runtime::process::{Spec, run};

use super::expectations::{Case, CaseSet};
use crate::memory::Memory;

/// The base revision of a case's diff when the case names none.
pub const DEFAULT_BASE: &str = "HEAD~1";

/// A loaded case set and the directory it came from.
#[derive(Debug, Clone)]
pub struct Fixtures {
  root: PathBuf,
  set: CaseSet,
}

/// Git's environment for a hermetic repository: no system or global config.
pub(super) fn git_env() -> Env {
  Env::process()
    .with("GIT_CONFIG_NOSYSTEM", "1")
    .with("GIT_CONFIG_GLOBAL", "/dev/null")
    .with("XDG_CONFIG_HOME", "/dev/null")
}

fn git(root: &Path, args: &[&str]) -> Result<(), String> {
  git_output(root, args).map(drop)
}

/// The trimmed stdout of `git args` in `root`.
fn git_output(root: &Path, args: &[&str]) -> Result<String, String> {
  let mut spec = Spec::new(["git"]);
  spec.argv.extend(args.iter().map(|arg| (*arg).to_owned()));
  spec.cwd = Some(root.to_path_buf());
  spec.env = Some(git_env());
  let output = run(&spec).map_err(|error| format!("git {args:?}: {error:?}"))?;
  if output.exit_code == 0 {
    Ok(output.stdout.trim().to_owned())
  } else {
    Err(format!("git {args:?}: {}", output.stderr.trim()))
  }
}

/// The commit `rev` names in the repository at `root`.
///
/// # Errors
/// Git fails or `rev` names nothing.
pub(super) fn rev_parse(root: &Path, rev: &str) -> Result<String, String> {
  git_output(root, &["rev-parse", "--verify", rev])
}

fn io(path: &Path, error: &std::io::Error) -> String {
  format!("{}: {error}", path.display())
}

/// Copy the tree `from` into `to`, creating directories as needed.
fn copy_tree(from: &Path, to: &Path) -> Result<(), String> {
  std::fs::create_dir_all(to).map_err(|error| io(to, &error))?;
  for entry in std::fs::read_dir(from).map_err(|error| io(from, &error))? {
    let entry = entry.map_err(|error| io(from, &error))?;
    let target = to.join(entry.file_name());
    let kind = entry
      .file_type()
      .map_err(|error| io(&entry.path(), &error))?;
    if kind.is_dir() {
      copy_tree(&entry.path(), &target)?;
    } else {
      std::fs::copy(entry.path(), &target).map_err(|error| io(&target, &error))?;
    }
  }
  Ok(())
}

/// Remove every work-tree entry except `.git`, so the next tree replaces the last.
fn clear_worktree(root: &Path) -> Result<(), String> {
  for entry in std::fs::read_dir(root).map_err(|error| io(root, &error))? {
    let entry = entry.map_err(|error| io(root, &error))?;
    if entry.file_name() == ".git" {
      continue;
    }
    let path = entry.path();
    let kind = entry.file_type().map_err(|error| io(&path, &error))?;
    let removed = if kind.is_dir() {
      std::fs::remove_dir_all(&path)
    } else {
      std::fs::remove_file(&path)
    };
    removed.map_err(|error| io(&path, &error))?;
  }
  Ok(())
}

impl Fixtures {
  /// Load `<root>/expectations.json` and check every case has its two trees.
  ///
  /// # Errors
  /// A missing or invalid expectations file, an unknown version, or a case whose
  /// `files/base` or `files/tip` directory is absent.
  pub fn load(root: &Path) -> Result<Fixtures, String> {
    let file = root.join("expectations.json");
    let text = std::fs::read_to_string(&file).map_err(|error| io(&file, &error))?;
    let set = CaseSet::parse(&text)?;
    let fixtures = Fixtures {
      root: root.to_path_buf(),
      set,
    };
    fixtures.check_trees()?;
    Ok(fixtures)
  }

  fn check_trees(&self) -> Result<(), String> {
    let trees = self
      .set
      .cases
      .iter()
      .flat_map(|case| ["base", "tip"].map(|side| (case, self.tree(case, side))));
    for (case, tree) in trees {
      if !tree.is_dir() {
        return Err(format!("case {}: missing {}", case.id, tree.display()));
      }
    }
    Ok(())
  }

  /// The loaded expectations.
  pub fn set(&self) -> &CaseSet {
    &self.set
  }

  /// The case's directory: `<root>/cases/<id>`.
  pub fn case_dir(&self, case: &Case) -> PathBuf {
    self.root.join("cases").join(&case.id)
  }

  fn tree(&self, case: &Case, side: &str) -> PathBuf {
    self.case_dir(case).join("files").join(side)
  }

  /// The revision the case's diff starts from.
  pub fn base_rev<'a>(&self, case: &'a Case) -> &'a str {
    case.base.as_deref().unwrap_or(DEFAULT_BASE)
  }

  /// The case's seeded review memory, when it has a `memory.json`.
  ///
  /// # Errors
  /// A present `memory.json` that cannot be read or parsed.
  pub fn memory(&self, case: &Case) -> Result<Option<Memory>, String> {
    let file = self.case_dir(case).join("memory.json");
    match std::fs::read_to_string(&file) {
      Ok(text) => serde_json::from_str(&text)
        .map(Some)
        .map_err(|error| format!("{}: {error}", file.display())),
      Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
      Err(error) => Err(io(&file, &error)),
    }
  }

  /// A temporary Git repository with the case's base tree committed, then its
  /// tip tree committed on top, so the case's base revision diffs to the tip.
  ///
  /// # Errors
  /// Git or the filesystem fails; the temporary directory is dropped with it.
  pub fn materialize_repo(&self, case: &Case) -> Result<TempDir, String> {
    let dir = tempfile::tempdir().map_err(|error| error.to_string())?;
    let root = dir.path();
    git(root, &["init", "-q", "-b", "main"])?;
    git(root, &["config", "user.name", "Eval"])?;
    git(root, &["config", "user.email", "eval@example.com"])?;
    for (side, message) in [("base", "base"), ("tip", "tip")] {
      clear_worktree(root)?;
      copy_tree(&self.tree(case, side), root)?;
      git(root, &["add", "-A"])?;
      git(root, &["commit", "-q", "-m", message])?;
    }
    Ok(dir)
  }
}

#[cfg(test)]
#[path = "tests/fixtures_test.rs"]
mod tests;
