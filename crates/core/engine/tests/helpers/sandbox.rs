//! A temporary root for registry tests: a home, a Codex home and a project, plus
//! file helpers that create parents. Helpers return `Res`; tests unwrap it.

use std::path::{Path, PathBuf};

/// A helper's result.
pub(crate) type Res<T> = Result<T, String>;

/// Directories under one temporary root, removed on drop.
pub(crate) struct Sandbox {
  _dir: tempfile::TempDir,
  pub(crate) root: PathBuf,
}

impl Sandbox {
  /// An empty sandbox with a canonical root.
  pub(crate) fn new() -> Res<Sandbox> {
    let dir = tempfile::tempdir().map_err(|err| err.to_string())?;
    let root = std::fs::canonicalize(dir.path()).map_err(|err| err.to_string())?;
    Ok(Sandbox { _dir: dir, root })
  }

  /// `root/<rel>`.
  pub(crate) fn path(&self, rel: &str) -> PathBuf {
    self.root.join(rel)
  }

  /// `root/<rel>` as text.
  pub(crate) fn text(&self, rel: &str) -> String {
    self.path(rel).to_string_lossy().into_owned()
  }
}

/// Write `body` to `path`, creating its parents.
pub(crate) fn write(path: &Path, body: &str) -> Res<()> {
  if let Some(parent) = path.parent() {
    std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
  }
  std::fs::write(path, body).map_err(|err| format!("{}: {err}", path.display()))
}
