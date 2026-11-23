//! Carry user-edited `settings/` files from the previous plugin version into
//! the new one. A host that caches `…/toolu/<version>/` leaves the old tree
//! in place; files the user did not change stay on the new release.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use super::super::load::is_file;

/// Records which previous version was already considered. A dotfile, so the
/// list loaders never read it.
const MIGRATED_FROM: &str = ".migrated-from";
/// Copying a plugin stamps files a moment apart. An edit has to land after that
/// window, or a file written last during the copy looks user-edited.
const EDIT_AFTER: Duration = Duration::from_secs(2);

/// Files copied out of a previous plugin version, when any were.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsMigration {
  /// The previous version the files came from.
  pub from: Option<String>,
  /// File names copied, sorted.
  pub kept: Vec<String>,
}

/// Copies settings the user edited under older `…/toolu/<version>/`
/// directories into `plugin_root`'s `settings/`. A root that is not a version directory,
/// and a version already recorded in `settings/.migrated-from`, do nothing.
///
/// # Errors
/// A previous tree or a destination file could not be read or written. The
/// marker is left unwritten, so a later call retries.
pub fn migrate_plugin_settings(plugin_root: &Path) -> Result<SettingsMigration, String> {
  let Some(current) = version_dir(plugin_root) else {
    return Ok(SettingsMigration {
      from: None,
      kept: Vec::new(),
    });
  };
  let marker = plugin_root.join("settings").join(MIGRATED_FROM);
  if is_file(&marker) {
    return Ok(SettingsMigration {
      from: None,
      kept: Vec::new(),
    });
  }
  let older = older_versions(plugin_root, current)?;
  if older.is_empty() {
    write_marker(&marker, "none")?;
    return Ok(SettingsMigration {
      from: None,
      kept: Vec::new(),
    });
  }
  let (from, kept) = carry_older(&older, plugin_root)?;
  let name = older
    .first()
    .and_then(|path| path.file_name())
    .and_then(OsStr::to_str)
    .unwrap_or("none");
  write_marker(&marker, name)?;
  Ok(SettingsMigration { from, kept })
}

/// Newest older sibling first. A version that was installed and never started
/// must not hide an edit in the version before it.
fn carry_older(
  older: &[PathBuf],
  plugin_root: &Path,
) -> Result<(Option<String>, Vec<String>), String> {
  let mut kept = Vec::new();
  let mut from = None;
  let mut mixed = false;
  for previous in older {
    let names = carry_installed_settings(previous, plugin_root)?;
    // This version already ran migration, so it is the user's current
    // settings. An older sibling must not restore a file they removed.
    let started = is_file(&previous.join("settings").join(MIGRATED_FROM));
    if !names.is_empty() {
      let version = previous
        .file_name()
        .and_then(OsStr::to_str)
        .unwrap_or("none")
        .to_owned();
      if from.is_some() {
        mixed = true;
      } else {
        from = Some(version);
      }
      kept.extend(names);
    }
    if started {
      break;
    }
  }
  if mixed {
    from = None;
  }
  kept.sort();
  Ok((from, kept))
}

/// Copies user-edited files from `previous`'s `settings/` onto `new_root`'s
/// `settings/`. `previous` is a plugin root (a version directory, or the
/// directory a local install is about to replace).
///
/// # Errors
/// A settings file could not be read or written.
pub fn carry_installed_settings(previous: &Path, new_root: &Path) -> Result<Vec<String>, String> {
  let from = previous.join("settings");
  if !from.is_dir() {
    return Ok(Vec::new());
  }
  let installed = installed_at(previous)?;
  let landed = installed_at(new_root).ok();
  let mut files = settings_files(&from)?;
  files.sort();
  let mut kept = Vec::new();
  for (name, path) in files {
    let dest = new_root.join("settings").join(&name);
    if copy_edited(&path, &dest, installed, landed)? {
      kept.push(name);
    }
  }
  Ok(kept)
}

/// `plugin_root` when it is `…/toolu/<MAJOR.MINOR.PATCH>`.
fn version_dir(plugin_root: &Path) -> Option<(u64, u64, u64)> {
  let parent_is_toolu = plugin_root.parent().and_then(Path::file_name) == Some(OsStr::new("toolu"));
  if !parent_is_toolu {
    return None;
  }
  plugin_root
    .file_name()
    .and_then(OsStr::to_str)
    .and_then(crate::version::triple)
}

/// Older siblings that have a `settings/` directory, newest first.
fn older_versions(plugin_root: &Path, current: (u64, u64, u64)) -> Result<Vec<PathBuf>, String> {
  let Some(parent) = plugin_root.parent() else {
    return Ok(Vec::new());
  };
  let mut found = Vec::new();
  for entry in read_dir(parent)? {
    let path = entry.path();
    if !is_real_dir(&path) {
      continue;
    }
    let Some(version) = entry.file_name().to_str().and_then(crate::version::triple) else {
      continue;
    };
    if version >= current || !is_real_dir(&path.join("settings")) {
      continue;
    }
    found.push((version, path));
  }
  found.sort_by_key(|item| std::cmp::Reverse(item.0));
  Ok(found.into_iter().map(|(_, path)| path).collect())
}

fn settings_files(dir: &Path) -> Result<Vec<(String, PathBuf)>, String> {
  let mut files = Vec::new();
  for entry in read_dir(dir)? {
    let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
      return Err(format!(
        "{}: file name is not utf-8",
        entry.path().display()
      ));
    };
    if name.starts_with('.') {
      continue;
    }
    let path = entry.path();
    if is_real_file(&path) {
      files.push((name, path));
    }
  }
  Ok(files)
}

/// The latest mtime outside `settings/`: the install, not a later edit inside it.
fn installed_at(plugin: &Path) -> Result<SystemTime, String> {
  let mut latest = None;
  note_tree(plugin, plugin, &mut latest)?;
  latest.ok_or_else(|| format!("{}: no install time", plugin.display()))
}

fn note_tree(plugin: &Path, dir: &Path, latest: &mut Option<SystemTime>) -> Result<(), String> {
  for entry in read_dir(dir)? {
    let path = entry.path();
    // Claude writes `.orphaned_at` and `.in_use` after the copy. Counting them
    // makes a later edit look like part of the install.
    if path == plugin.join("settings")
      || path == plugin.join(".orphaned_at")
      || path == plugin.join(".in_use")
    {
      continue;
    }
    if is_real_dir(&path) {
      note_tree(plugin, &path, latest)?;
    } else if is_real_file(&path) {
      note_mtime(&path, latest)?;
    }
  }
  Ok(())
}

fn note_mtime(path: &Path, latest: &mut Option<SystemTime>) -> Result<(), String> {
  let modified = mtime(path)?;
  if latest.is_none_or(|have| modified > have) {
    *latest = Some(modified);
  }
  Ok(())
}

/// `true` when `old` is missing from `new`, or was edited after its install
/// and `new` was not. `landed` is the new tree's install time.
fn copy_edited(
  old: &Path,
  new: &Path,
  installed: SystemTime,
  landed: Option<SystemTime>,
) -> Result<bool, String> {
  let old_bytes = std::fs::read(old).map_err(|err| format!("{}: {err}", old.display()))?;
  if is_file(new) && !replaces(old, new, &old_bytes, installed, landed)? {
    return Ok(false);
  }
  let text = String::from_utf8(old_bytes).map_err(|err| format!("{}: {err}", old.display()))?;
  crate::atomic::replace_atomic(new, &text)
    .map_err(|err| format!("cannot write {}: {err}", new.display()))?;
  // A copy has to stay newer than the tree it lands in, or the next update
  // reads that copy as part of the install and drops it.
  stamp_kept(new, mtime(old)?, landed)?;
  Ok(true)
}

fn replaces(
  old: &Path,
  new: &Path,
  old_bytes: &[u8],
  installed: SystemTime,
  landed: Option<SystemTime>,
) -> Result<bool, String> {
  let new_bytes = std::fs::read(new).map_err(|err| format!("{}: {err}", new.display()))?;
  if old_bytes == new_bytes || edited_since(new, landed)? {
    return Ok(false);
  }
  edited_since(old, Some(installed))
}

fn edited_since(path: &Path, installed: Option<SystemTime>) -> Result<bool, String> {
  let Some(installed) = installed else {
    return Ok(false);
  };
  let modified = mtime(path)?;
  Ok(
    installed
      .checked_add(EDIT_AFTER)
      .is_some_and(|limit| modified > limit),
  )
}

fn stamp_kept(path: &Path, source: SystemTime, landed: Option<SystemTime>) -> Result<(), String> {
  let file = std::fs::OpenOptions::new()
    .write(true)
    .open(path)
    .map_err(|err| format!("{}: {err}", path.display()))?;
  file
    .set_modified(kept_mtime(source, landed))
    .map_err(|err| format!("{}: {err}", path.display()))
}

fn kept_mtime(source: SystemTime, landed: Option<SystemTime>) -> SystemTime {
  let Some(floor) = landed.and_then(|time| {
    time
      .checked_add(EDIT_AFTER)
      .and_then(|after| after.checked_add(Duration::from_secs(1)))
  }) else {
    return source;
  };
  if source > floor { source } else { floor }
}

fn write_marker(path: &Path, version: &str) -> Result<(), String> {
  crate::atomic::replace_atomic(path, &format!("{version}\n"))
    .map_err(|err| format!("cannot write {}: {err}", path.display()))
}

fn mtime(path: &Path) -> Result<SystemTime, String> {
  std::fs::metadata(path)
    .and_then(|meta| meta.modified())
    .map_err(|err| format!("{}: {err}", path.display()))
}

fn read_dir(dir: &Path) -> Result<Vec<std::fs::DirEntry>, String> {
  std::fs::read_dir(dir)
    .map_err(|err| format!("{}: {err}", dir.display()))?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|err| format!("{}: {err}", dir.display()))
}

fn is_real_dir(path: &Path) -> bool {
  std::fs::symlink_metadata(path).is_ok_and(|meta| meta.file_type().is_dir())
}

fn is_real_file(path: &Path) -> bool {
  std::fs::symlink_metadata(path).is_ok_and(|meta| meta.file_type().is_file())
}

#[cfg(test)]
#[path = "tests/settings_migrate_test.rs"]
mod tests;
