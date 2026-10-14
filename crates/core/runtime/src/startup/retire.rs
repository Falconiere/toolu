//! Former stable paths (#440). Until #440 a `SessionStart` hook linked a plugin
//! script at `<config root>/<dir>/<name>` (`jev/jev.sh`, `statusline/statusline.sh`,
//! `toolu-review/write-state.sh`) so the agent's shell could run it. Every surface
//! now runs `toolu …`, so the hook removes the link it made. A regular file, a
//! directory, or a link toolu did not make is the user's and is never touched.

use std::ffi::OsStr;
use std::path::Path;

use crate::host::roots::Roots;

/// What retiring a former stable path did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Retired {
  /// Nothing was there.
  Absent,
  /// toolu's link was removed.
  Removed,
  /// The user's file, directory or link holds the path and was left alone.
  Kept,
  /// toolu's link could not be removed: the warning line.
  Failed(String),
}

/// Whether `target` is what toolu linked, or `also`: `<plugin root>/scripts/<name>`,
/// where a plugin root lies under a `plugins` directory (the hosts' `plugins/cache/…`,
/// or a checkout's `plugins/<plugin>`).
fn ours(target: &Path, name: &str, also: Option<&Path>) -> bool {
  let scripts = target
    .parent()
    .filter(|dir| dir.file_name() == Some(OsStr::new("scripts")));
  let script = target.file_name() == Some(OsStr::new(name))
    && scripts.is_some_and(|dir| {
      dir
        .ancestors()
        .any(|up| up.file_name() == Some(OsStr::new("plugins")))
    });
  script || also.is_some_and(|other| other == target)
}

/// Remove `<config root>/<dir>/<name>` when it is a link toolu published to a
/// plugin's `scripts/<name>`, or to `also`.
pub fn retire(roots: &Roots, plugin: &str, dir: &str, name: &str, also: Option<&Path>) -> Retired {
  let path = roots.config_root().join(dir).join(name);
  let Ok(meta) = std::fs::symlink_metadata(&path) else {
    return Retired::Absent;
  };
  if !meta.file_type().is_symlink() {
    return Retired::Kept;
  }
  match std::fs::read_link(&path) {
    Ok(target) if ours(&target, name, also) => match std::fs::remove_file(&path) {
      Ok(()) => Retired::Removed,
      Err(error) => Retired::Failed(format!(
        "{plugin}: cannot remove the retired {}: {error}",
        path.display()
      )),
    },
    Ok(_) | Err(_) => Retired::Kept,
  }
}

#[cfg(test)]
#[path = "tests/retire_test.rs"]
mod tests;
