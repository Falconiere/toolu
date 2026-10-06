//! Scratch directories, real git and environments for the detect tests.

use std::path::{Path, PathBuf};
use std::process::Command;

use toolu_runtime::env::Env;

/// A helper's result; the `#[test]` functions return it.
pub(crate) type Res<T> = Result<T, String>;

/// A scratch directory with an isolated `HOME`; `root` is canonical.
pub(crate) struct Scratch {
  _dir: tempfile::TempDir,
  pub(crate) root: PathBuf,
  pub(crate) home: PathBuf,
}

impl Scratch {
  pub(crate) fn new() -> Res<Scratch> {
    let dir = tempfile::tempdir().map_err(|err| err.to_string())?;
    let root = std::fs::canonicalize(dir.path()).map_err(|err| err.to_string())?;
    let home = root.join("home");
    mkdir(&home)?;
    Ok(Scratch {
      _dir: dir,
      root,
      home,
    })
  }

  /// The process `PATH`, which finds git.
  pub(crate) fn system_path() -> Res<String> {
    std::env::var("PATH").map_err(|err| err.to_string())
  }

  /// An environment holding `path` and this scratch's `HOME`.
  pub(crate) fn env_with(&self, path: &str) -> Env {
    Env::from_pairs([
      ("PATH", path),
      ("HOME", &self.home.display().to_string()),
      ("LC_ALL", "C"),
    ])
  }

  /// An environment whose `PATH` finds git.
  pub(crate) fn env(&self) -> Res<Env> {
    Ok(self.env_with(&Scratch::system_path()?))
  }

  /// `git <args>` in `cwd`, which must succeed.
  pub(crate) fn git(&self, cwd: &Path, args: &[&str]) -> Res<()> {
    let user = [
      "-c",
      "user.name=t",
      "-c",
      "user.email=t@t",
      "-c",
      "commit.gpgsign=false",
    ];
    let status = Command::new("git")
      .args(user)
      .args(args)
      .current_dir(cwd)
      .env_clear()
      .env("PATH", Scratch::system_path()?)
      .env("HOME", &self.home)
      .env("GIT_CONFIG_NOSYSTEM", "1")
      .status()
      .map_err(|err| format!("git {args:?}: {err}"))?;
    status
      .success()
      .then_some(())
      .ok_or_else(|| format!("git {args:?} failed in {}", cwd.display()))
  }
}

/// `dir` and its parents.
pub(crate) fn mkdir(dir: &Path) -> Res<()> {
  std::fs::create_dir_all(dir).map_err(|err| format!("{}: {err}", dir.display()))
}

/// `body` at `path`, its parents created.
pub(crate) fn write(path: &Path, body: &[u8]) -> Res<()> {
  if let Some(parent) = path.parent() {
    mkdir(parent)?;
  }
  std::fs::write(path, body).map_err(|err| format!("{}: {err}", path.display()))
}
