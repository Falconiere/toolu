//! Carry user-edited `settings/` files into the next plugin version.

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use super::super::load::is_file;
use crate::atomic::replace_atomic;

const EDIT_AFTER: Duration = Duration::from_secs(2);

type Out<T> = Result<T, String>;

/// Files copied out of a previous plugin version, when any were.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SettingsMigration {
  /// The one previous version the files came from.
  pub from: Option<String>,
  /// File names copied, sorted.
  pub kept: Vec<String>,
}

/// Copies edited settings from older `…/toolu/<version>/` trees into `plugin_root`.
/// A non-version root, or one already recorded beside that directory, does nothing.
///
/// # Errors
/// A read or write error leaves the marker unwritten, so the next call retries.
pub fn migrate_plugin_settings(plugin_root: &Path) -> Out<SettingsMigration> {
  let Some(current) = version_dir(plugin_root) else {
    return Ok(SettingsMigration::default());
  };
  let marker = migrated_marker(plugin_root);
  if is_file(&marker) {
    return Ok(SettingsMigration::default());
  }
  let older = older_versions(plugin_root, current)?;
  let Some(nearest) = older.first() else {
    write_marker(&marker, "none")?;
    return Ok(SettingsMigration::default());
  };
  let (from, kept) = carry_older(&older, plugin_root)?;
  write_marker(&marker, &file_name(nearest))?;
  Ok(SettingsMigration { from, kept })
}

fn carry_older(older: &[PathBuf], root: &Path) -> Out<(Option<String>, Vec<String>)> {
  let mut kept = Vec::new();
  let mut from = None;
  let mut mixed = false;
  for previous in older {
    let names = carry_installed_settings(previous, root)?;
    // A started sibling is the user's settings. Do not restore what they removed.
    let started = is_file(&migrated_marker(previous));
    if !names.is_empty() {
      mixed |= from.is_some();
      from.get_or_insert_with(|| file_name(previous));
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

/// Copies user-edited files from `previous`'s `settings/` onto `new_root`'s.
///
/// # Errors
/// A settings file could not be read or written.
pub fn carry_installed_settings(previous: &Path, new_root: &Path) -> Out<Vec<String>> {
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

fn version_dir(plugin_root: &Path) -> Option<(u64, u64, u64)> {
  let parent = plugin_root.parent().and_then(Path::file_name);
  parent
    .filter(|name| *name == OsStr::new("toolu"))
    .and(file_str(plugin_root))
    .and_then(crate::version::triple)
}

fn older_versions(plugin_root: &Path, current: (u64, u64, u64)) -> Out<Vec<PathBuf>> {
  let Some(parent) = plugin_root.parent() else {
    return Ok(Vec::new());
  };
  let mut found = Vec::new();
  for entry in read_dir(parent)? {
    let path = entry.path();
    let Some(version) = file_str(&path).and_then(crate::version::triple) else {
      continue;
    };
    if is_real_dir(&path) && version < current && is_real_dir(&path.join("settings")) {
      found.push((version, path));
    }
  }
  found.sort_by_key(|item| std::cmp::Reverse(item.0));
  Ok(found.into_iter().map(|(_, path)| path).collect())
}

fn settings_files(dir: &Path) -> Out<Vec<(String, PathBuf)>> {
  let mut files = Vec::new();
  for entry in read_dir(dir)? {
    let path = entry.path();
    let Some(name) = file_str(&path).map(str::to_owned) else {
      return Err(shown(&path, "file name is not utf-8"));
    };
    if !name.starts_with('.') && is_real_file(&path) {
      files.push((name, path));
    }
  }
  Ok(files)
}

fn installed_at(plugin: &Path) -> Out<SystemTime> {
  let mut latest = None;
  note_tree(plugin, plugin, &mut latest)?;
  latest.ok_or_else(|| shown(plugin, "no install time"))
}

fn note_tree(plugin: &Path, dir: &Path, latest: &mut Option<SystemTime>) -> Out<()> {
  for entry in read_dir(dir)? {
    let path = entry.path();
    // Claude writes these after the copy. Counting them hides a later edit.
    if path.parent() == Some(plugin) && host_owned(file_str(&path)) {
      continue;
    }
    if is_real_dir(&path) {
      note_tree(plugin, &path, latest)?;
    } else if is_real_file(&path) {
      let modified = mtime(&path)?;
      if latest.is_none_or(|have| modified > have) {
        *latest = Some(modified);
      }
    }
  }
  Ok(())
}

fn host_owned(name: Option<&str>) -> bool {
  matches!(name, Some("settings" | ".orphaned_at" | ".in_use"))
}

fn copy_edited(
  old: &Path,
  new: &Path,
  installed: SystemTime,
  landed: Option<SystemTime>,
) -> Out<bool> {
  let bytes = read(old)?;
  if is_file(new) && !replaces(old, new, &bytes, installed, landed)? {
    return Ok(false);
  }
  let text = io(old, String::from_utf8(bytes))?;
  io(new, replace_atomic(new, &text))?;
  // Stay newer than the tree it lands in, or the next update drops the copy.
  let file = io(new, fs::OpenOptions::new().write(true).open(new))?;
  io(new, file.set_modified(kept_mtime(mtime(old)?, landed)))?;
  Ok(true)
}

fn replaces(
  old: &Path,
  new: &Path,
  old_bytes: &[u8],
  installed: SystemTime,
  landed: Option<SystemTime>,
) -> Out<bool> {
  if read(new)? == old_bytes || edited_since(new, landed)? {
    return Ok(false);
  }
  edited_since(old, Some(installed))
}

fn edited_since(path: &Path, installed: Option<SystemTime>) -> Out<bool> {
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

fn kept_mtime(source: SystemTime, landed: Option<SystemTime>) -> SystemTime {
  let floor = landed.and_then(|time| {
    time
      .checked_add(EDIT_AFTER)
      .and_then(|after| after.checked_add(Duration::from_secs(1)))
  });
  floor.map_or(source, |floor| source.max(floor))
}

fn write_marker(path: &Path, version: &str) -> Out<()> {
  let text = format!("{version}\n");
  io(path, replace_atomic(path, &text))
}

fn read(path: &Path) -> Out<Vec<u8>> {
  io(path, fs::read(path))
}

fn mtime(path: &Path) -> Out<SystemTime> {
  io(path, fs::metadata(path).and_then(|meta| meta.modified()))
}

fn read_dir(dir: &Path) -> Out<Vec<fs::DirEntry>> {
  let opened = io(dir, fs::read_dir(dir))?;
  io(dir, opened.collect::<Result<Vec<_>, _>>())
}

fn io<T>(path: &Path, result: Result<T, impl std::fmt::Display>) -> Out<T> {
  result.map_err(|err| shown(path, &err.to_string()))
}

fn shown(path: &Path, err: &str) -> String {
  format!("{}: {err}", path.display())
}

fn migrated_marker(plugin: &Path) -> PathBuf {
  let name = file_str(plugin).unwrap_or("none");
  plugin.with_file_name(format!(".{name}.migrated-from"))
}

fn file_name(path: &Path) -> String {
  file_str(path).unwrap_or("none").to_owned()
}

fn file_str(path: &Path) -> Option<&str> {
  path.file_name().and_then(OsStr::to_str)
}

fn is_real_dir(path: &Path) -> bool {
  fs::symlink_metadata(path).is_ok_and(|meta| meta.is_dir())
}

fn is_real_file(path: &Path) -> bool {
  fs::symlink_metadata(path).is_ok_and(|meta| meta.is_file())
}

#[cfg(test)]
#[path = "tests/settings_migrate_test.rs"]
mod tests;
