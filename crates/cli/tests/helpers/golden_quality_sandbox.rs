//! A disposable root holding a git project, a `HOME` and a Codex home, the
//! sandbox each golden case runs in.

use std::path::{Component, Path, PathBuf};
use std::process::Command;

use crate::Res;
use crate::cases::Host;

/// A temporary project, `HOME` and Codex home under one canonical root.
pub(crate) struct Sandbox {
  _dir: tempfile::TempDir,
  /// The canonical root; the text every output replaces with `<ROOT>`.
  pub(crate) root: PathBuf,
  /// The git project the hooks run in.
  pub(crate) project: PathBuf,
  /// The user's home.
  pub(crate) home: PathBuf,
  /// The Codex home, `<home>/.codex`.
  pub(crate) codex_home: PathBuf,
}

/// `rel` under `base`; a path that leaves `base` is an error.
pub(crate) fn inside(base: &Path, rel: &str) -> Res<PathBuf> {
  let path = Path::new(rel);
  let plain = path
    .components()
    .all(|part| matches!(part, Component::Normal(_) | Component::CurDir));
  if !plain {
    return Err(format!("sandbox path escapes {}: {rel}", base.display()).into());
  }
  Ok(base.join(path))
}

/// Run `git -C <dir> <args>` and return its stdout.
pub(crate) fn git(dir: &Path, args: &[&str]) -> Res<String> {
  // A repository named by the caller's environment (a git hook runs us) is not ours.
  let out = Command::new("git")
    .arg("-C")
    .arg(dir)
    .args(args)
    .env_remove("GIT_DIR")
    .env_remove("GIT_WORK_TREE")
    .env_remove("GIT_INDEX_FILE")
    .env_remove("GIT_COMMON_DIR")
    .output()?;
  if !out.status.success() {
    let stderr = String::from_utf8_lossy(&out.stderr);
    return Err(format!("git {args:?} exited {}: {}", out.status, stderr.trim()).into());
  }
  Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

impl Sandbox {
  /// A sandbox whose project is a git repository with one commit on `main`.
  pub(crate) fn new() -> Res<Self> {
    let dir = tempfile::tempdir()?;
    // The canonical root: macOS temp directories sit behind a symlink.
    let root = std::fs::canonicalize(dir.path())?;
    let project = root.join("project");
    let home = root.join("home");
    let codex_home = home.join(".codex");
    std::fs::create_dir_all(&project)?;
    std::fs::create_dir_all(&codex_home)?;
    git(&project, &["init", "-q", "-b", "main"])?;
    git(&project, &["config", "user.email", "harness@toolu.test"])?;
    git(&project, &["config", "user.name", "toolu harness"])?;
    git(&project, &["config", "commit.gpgsign", "false"])?;
    std::fs::write(project.join(".gitkeep"), "")?;
    git(&project, &["add", "-A"])?;
    git(&project, &["commit", "-q", "-m", "harness: initial commit"])?;
    Ok(Self {
      _dir: dir,
      root,
      project,
      home,
      codex_home,
    })
  }

  /// `rel` under the project.
  pub(crate) fn path(&self, rel: &str) -> Res<PathBuf> {
    inside(&self.project, rel)
  }

  /// Write `body` to `rel` under the project, creating parents.
  pub(crate) fn write(&self, rel: &str, body: &str) -> Res<PathBuf> {
    write_at(&self.path(rel)?, body)?;
    self.path(rel)
  }

  /// Run git in the project.
  pub(crate) fn git(&self, args: &[&str]) -> Res<String> {
    git(&self.project, args)
  }

  /// The project's config directory for `host`.
  pub(crate) fn project_config(&self, host: Host) -> PathBuf {
    self.project.join(match host {
      Host::Claude => ".claude",
      Host::Codex => ".codex",
    })
  }

  /// The user's config root for `host`, where the registry lives.
  pub(crate) fn user_config(&self, host: Host) -> PathBuf {
    match host {
      Host::Claude => self.home.join(".claude"),
      Host::Codex => self.codex_home.clone(),
    }
  }
}

/// Write `body` at `path`, creating parents.
pub(crate) fn write_at(path: &Path, body: &str) -> Res<()> {
  if let Some(parent) = path.parent() {
    std::fs::create_dir_all(parent)?;
  }
  std::fs::write(path, body)?;
  Ok(())
}
