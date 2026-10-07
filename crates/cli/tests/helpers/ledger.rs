//! A project for the `toolu ledger` black-box tests: a git repository on
//! `feat/x` one commit (`a.ts`) ahead of `main`, a home beside it, and the
//! environment every command of a test runs with (Claude host,
//! `PUSH_REVIEW_BASE=main`, a committer identity), the one
//! `plugins/toolu/hooks/src/__tests__/ledger-cases.ts` gives the Bun suite.

use std::error::Error;
use std::path::{Path, PathBuf};
use std::process::Command;

/// A helper's result; the tests unwrap it.
pub(crate) type Res<T> = Result<T, Box<dyn Error>>;

/// The `toolu` binary under test.
pub(crate) const TOOLU: &str = env!("CARGO_BIN_EXE_toolu");

/// The project and the temporary root that holds it.
pub(crate) struct Project {
  _dir: tempfile::TempDir,
  /// The repository.
  pub(crate) root: PathBuf,
  /// `HOME` and the toolu config dir.
  pub(crate) home: PathBuf,
}

impl Project {
  /// `main` with `base.txt`, then `feat/x` adding `a.ts`.
  pub(crate) fn new() -> Res<Project> {
    let dir = tempfile::tempdir()?;
    let base = std::fs::canonicalize(dir.path())?;
    let (root, home) = (base.join("project"), base.join("home"));
    std::fs::create_dir_all(&root)?;
    std::fs::create_dir_all(&home)?;
    let project = Project {
      _dir: dir,
      root,
      home,
    };
    project.sh(
      "git init -q -b main && echo base > base.txt && git add . && git commit -qm base \
       && git checkout -q -b feat/x && echo x > a.ts && git add a.ts && git commit -qm a",
    )?;
    Ok(project)
  }

  /// The environment of every command.
  pub(crate) fn env(&self) -> Vec<(String, String)> {
    let home = self.home.display().to_string();
    let pairs = [
      ("PATH", std::env::var("PATH").unwrap_or_default()),
      ("HOME", home.clone()),
      ("TOOLU_CONFIG_DIR", home),
      ("TOOLU_HOST_OVERRIDE", "claude".to_owned()),
      ("PUSH_REVIEW_BASE", "main".to_owned()),
      ("GIT_AUTHOR_NAME", "t".to_owned()),
      ("GIT_AUTHOR_EMAIL", "t@t".to_owned()),
      ("GIT_COMMITTER_NAME", "t".to_owned()),
      ("GIT_COMMITTER_EMAIL", "t@t".to_owned()),
      ("GIT_CONFIG_NOSYSTEM", "1".to_owned()),
    ];
    pairs
      .into_iter()
      .map(|(key, value)| (key.to_owned(), value))
      .collect()
  }

  /// `bash -c script` in the repository; a failing script is an error.
  pub(crate) fn sh(&self, script: &str) -> Res<String> {
    let output = Command::new("bash")
      .args(["-c", script])
      .current_dir(&self.root)
      .env_clear()
      .envs(self.env())
      .output()?;
    if !output.status.success() {
      return Err(format!("{script}: {}", String::from_utf8_lossy(&output.stderr)).into());
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
  }

  /// Writes `body` to `rel` under the repository.
  pub(crate) fn write(&self, rel: &str, body: &str) -> Res<()> {
    let path = self.root.join(rel);
    if let Some(dir) = path.parent() {
      std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, body)?;
    Ok(())
  }

  /// `toolu`, not yet run, in `cwd` with the project's environment.
  pub(crate) fn command(&self, cwd: &Path) -> Command {
    let mut command = Command::new(TOOLU);
    command.current_dir(cwd).env_clear().envs(self.env());
    command
  }

  /// The branch ledger's file.
  pub(crate) fn ledger(&self) -> PathBuf {
    self.root.join(".claude/tmp/plan-ledger/feat_x.json")
  }
}
