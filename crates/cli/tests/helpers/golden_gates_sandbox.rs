//! A disposable sandbox: a temp project (a git repository with one commit), a
//! home and the host config roots, owned by one value.

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

use crate::golden_gates_model::{Host, Res, Scope};

/// The sandbox of one case.
pub(crate) struct Sandbox {
  _dir: tempfile::TempDir,
  /// The real path of the sandbox root.
  pub(crate) root: PathBuf,
  /// The repository.
  pub(crate) project: PathBuf,
  /// `HOME`.
  pub(crate) home: PathBuf,
  /// `CODEX_HOME`.
  pub(crate) codex_home: PathBuf,
}

/// Remove the variables a surrounding git hook exports, so a sandbox repository is its own.
pub(crate) fn scrub_git_env(command: &mut Command) {
  for (key, _) in std::env::vars_os() {
    if key.to_string_lossy().starts_with("GIT_") {
      command.env_remove(key);
    }
  }
}

/// Run git in `dir` and return its stdout.
pub(crate) fn git_in<S: AsRef<OsStr>>(dir: &Path, args: &[S]) -> Res<String> {
  let mut command = Command::new("git");
  command.arg("-C").arg(dir).args(args);
  scrub_git_env(&mut command);
  let output = command.output()?;
  if !output.status.success() {
    let words: Vec<String> = args
      .iter()
      .map(|arg| arg.as_ref().to_string_lossy().into_owned())
      .collect();
    return Err(
      format!(
        "git {} exited {:?}: {}",
        words.join(" "),
        output.status.code(),
        String::from_utf8_lossy(&output.stderr).trim()
      )
      .into(),
    );
  }
  Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Write `text` at `path`, creating its directories.
pub(crate) fn write_at(path: &Path, text: &str) -> Res<()> {
  if let Some(parent) = path.parent() {
    fs::create_dir_all(parent)?;
  }
  fs::write(path, text)?;
  Ok(())
}

impl Sandbox {
  /// A sandbox whose project is a repository on `branch` with one commit.
  pub(crate) fn new(branch: &str) -> Res<Sandbox> {
    let dir = tempfile::Builder::new()
      .prefix("toolu-harness-")
      .tempdir()?;
    // macOS temp dirs sit behind a symlink; hooks that resolve their cwd see the real path.
    let root = fs::canonicalize(dir.path())?;
    let project = root.join("project");
    let home = root.join("home");
    let codex_home = home.join(".codex");
    fs::create_dir_all(&project)?;
    fs::create_dir_all(&codex_home)?;
    git_in(&project, &["init", "-q", "-b", branch])?;
    for (key, value) in [
      ("user.email", "harness@toolu.test"),
      ("user.name", "toolu harness"),
      ("commit.gpgsign", "false"),
      ("maintenance.auto", "false"),
      ("gc.auto", "0"),
    ] {
      git_in(&project, &["config", key, value])?;
    }
    write_at(&project.join(".gitkeep"), "")?;
    git_in(&project, &["add", "-A"])?;
    git_in(&project, &["commit", "-q", "-m", "harness: initial commit"])?;
    Ok(Sandbox {
      _dir: dir,
      root,
      project,
      home,
      codex_home,
    })
  }

  /// Run git in the project.
  pub(crate) fn git<S: AsRef<OsStr>>(&self, args: &[S]) -> Res<String> {
    git_in(&self.project, args)
  }

  /// The host's config directory at `scope`.
  pub(crate) fn config_dir(&self, host: Host, scope: Scope) -> PathBuf {
    match (scope, host) {
      (Scope::Project, host) => self.project.join(host.state_dir()),
      (Scope::User, Host::Claude) => self.home.join(".claude"),
      (Scope::User, Host::Codex) => self.codex_home.clone(),
    }
  }

  /// Write `body` as the host's `toolu.config.json`, formatted as `JSON.stringify(body, null, 2)`.
  pub(crate) fn write_config(&self, host: Host, scope: Scope, body: &Value) -> Res<()> {
    let text = format!("{}\n", serde_json::to_string_pretty(body)?);
    write_at(
      &self.config_dir(host, scope).join("toolu.config.json"),
      &text,
    )
  }
}
