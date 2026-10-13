//! Whole-file writes through a temporary file and a rename in the target's
//! directory, as `writeAtomic` did: a symlinked config is written at its target
//! so the link survives, and a rewrite keeps the file's permission bits.

use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};

/// The path a write lands on: a symlink's target, else `path`.
fn target(path: &Path) -> std::io::Result<PathBuf> {
  match fs::canonicalize(path) {
    Ok(real) => Ok(real),
    Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(path.to_path_buf()),
    Err(err) => Err(err),
  }
}

/// Writes `text` to `path` atomically.
pub(crate) fn write_atomic(path: &Path, text: &str) -> std::io::Result<()> {
  let target = target(path)?;
  let dir = target.parent().map(Path::to_path_buf).unwrap_or_default();
  fs::create_dir_all(&dir)?;
  let mode = match fs::metadata(&target) {
    Ok(meta) => Some(meta.permissions().mode() & 0o7777),
    Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
    Err(err) => return Err(err),
  };
  let name = target.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
  let temp = dir.join(format!(".{name}.{}.tmp", std::process::id()));
  let written = (|| {
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    if let Some(mode) = mode {
      options.mode(mode);
    }
    let mut file = options.open(&temp)?;
    file.write_all(text.as_bytes())?;
    if let Some(mode) = mode {
      fs::set_permissions(&temp, fs::Permissions::from_mode(mode))?;
    }
    fs::rename(&temp, &target)
  })();
  if written.is_err() {
    let _removed = fs::remove_file(&temp);
  }
  written
}

#[cfg(test)]
#[path = "tests/write_test.rs"]
mod tests;
