//! A sandbox like the TypeScript harness's `createSandbox`: a `project` (a git
//! repository with one commit when a branch is given) and a `home` under one
//! temporary root, and the hook environment bound to them.

use std::path::PathBuf;
use std::process::Command;

use toolu_runtime::env::Env;

/// A helper's result; the `#[test]` functions unwrap it.
pub(crate) type Res<T> = Result<T, String>;

/// A temporary root holding `project` and `home`.
pub(crate) struct Sandbox {
  _dir: tempfile::TempDir,
  pub(crate) project: PathBuf,
  pub(crate) home: PathBuf,
}

impl Sandbox {
  /// A sandbox; with `branch`, `project` is a repository on it with one commit.
  pub(crate) fn new(branch: Option<&str>) -> Res<Sandbox> {
    let dir = tempfile::tempdir().map_err(|err| err.to_string())?;
    let root = std::fs::canonicalize(dir.path()).map_err(|err| err.to_string())?;
    let (project, home) = (root.join("project"), root.join("home"));
    for made in [&project, &home] {
      std::fs::create_dir_all(made).map_err(|err| err.to_string())?;
    }
    let sandbox = Sandbox {
      _dir: dir,
      project,
      home,
    };
    if let Some(branch) = branch {
      sandbox.git(&["init", "-q", "-b", branch])?;
      sandbox.git(&[
        "commit",
        "-q",
        "--allow-empty",
        "-m",
        "harness: initial commit",
      ])?;
    }
    Ok(sandbox)
  }

  /// `git <args>` in `project` with an isolated HOME; its stdout.
  pub(crate) fn git(&self, args: &[&str]) -> Res<String> {
    let out = Command::new("git")
      .args([
        "-c",
        "user.name=toolu harness",
        "-c",
        "user.email=harness@toolu.test",
      ])
      .args(["-c", "commit.gpgsign=false"])
      .args(args)
      .current_dir(&self.project)
      .env("HOME", &self.home)
      .env("GIT_CONFIG_NOSYSTEM", "1")
      .output()
      .map_err(|err| err.to_string())?;
    if !out.status.success() {
      return Err(format!(
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
      ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
  }

  /// The hook environment: HOME, the project, the Claude host, and PATH.
  pub(crate) fn env(&self) -> Env {
    Env::from_pairs([
      ("PATH", std::env::var("PATH").unwrap_or_default()),
      ("HOME", self.home.display().to_string()),
      ("TOOLU_PROJECT_DIR", self.project.display().to_string()),
      ("TOOLU_HOST_OVERRIDE", "claude".to_owned()),
    ])
  }
}
