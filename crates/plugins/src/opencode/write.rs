//! Whole-file writes through a temporary file and a rename in the target's
//! directory, as `writeAtomic` did: a symlinked config is written at its target
//! so the link survives, and a rewrite keeps the file's permission bits. The
//! temporary file is created exclusively (`O_EXCL`), so a symlink planted at its
//! name is never followed. A write is staged first and committed after, so a
//! run can stage every file before it renames any.

use std::fs::{self, File, OpenOptions};
use std::io::Write as _;
use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

/// Distinguishes the temporary files of one process.
static NEXT: AtomicU32 = AtomicU32::new(0);

/// How many temporary names a stage tries before it gives up.
const ATTEMPTS: u32 = 16;

/// A file written beside its target, not yet renamed onto it.
#[derive(Debug)]
pub(crate) struct Staged {
  temp: PathBuf,
  target: PathBuf,
}

/// The path a write lands on: a symlink's target, else `path`.
fn target(path: &Path) -> std::io::Result<PathBuf> {
  match fs::canonicalize(path) {
    Ok(real) => Ok(real),
    Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(path.to_path_buf()),
    Err(err) => Err(err),
  }
}

/// A new, exclusively created temporary file beside `target`.
fn create(dir: &Path, name: &str, mode: Option<u32>) -> std::io::Result<(PathBuf, File)> {
  let mut last = std::io::Error::other("no temporary name tried");
  for _ in 0..ATTEMPTS {
    let unique = NEXT.fetch_add(1, Ordering::Relaxed);
    let temp = dir.join(format!(".{name}.{}.{unique}.tmp", std::process::id()));
    let mut options = OpenOptions::new();
    options
      .write(true)
      .create_new(true)
      .mode(mode.unwrap_or(0o644));
    match options.open(&temp) {
      Ok(file) => return Ok((temp, file)),
      Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => last = err,
      Err(err) => return Err(err),
    }
  }
  Err(last)
}

/// Writes `text` beside `path`'s target, keeping the target's mode.
pub(crate) fn stage(path: &Path, text: &str) -> std::io::Result<Staged> {
  let target = target(path)?;
  let dir = target.parent().map(Path::to_path_buf).unwrap_or_default();
  fs::create_dir_all(&dir)?;
  let mode = match fs::metadata(&target) {
    Ok(meta) => Some(meta.permissions().mode() & 0o7777),
    Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
    Err(err) => return Err(err),
  };
  let name = target
    .file_name()
    .map(|name| name.to_string_lossy().into_owned())
    .unwrap_or_default();
  let (temp, mut file) = create(&dir, &name, mode)?;
  let written = file.write_all(text.as_bytes()).and_then(|()| match mode {
    Some(mode) => file.set_permissions(fs::Permissions::from_mode(mode)),
    None => Ok(()),
  });
  let staged = Staged { temp, target };
  match written {
    Ok(()) => Ok(staged),
    Err(err) => {
      staged.discard();
      Err(err)
    }
  }
}

impl Staged {
  /// Renames the staged file onto its target.
  pub(crate) fn commit(self) -> std::io::Result<()> {
    let renamed = fs::rename(&self.temp, &self.target);
    if renamed.is_err() {
      self.discard();
    }
    renamed
  }

  /// Removes the staged file.
  pub(crate) fn discard(&self) {
    let _removed = fs::remove_file(&self.temp);
  }
}

#[cfg(test)]
#[path = "tests/write_test.rs"]
mod tests;
