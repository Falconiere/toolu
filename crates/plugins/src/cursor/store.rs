//! The directory Cursor loads local plugins from, and the plugins toolu put in it. A
//! plugin directory toolu installed carries a marker file; one without it is the
//! user's and is never touched.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use toolu_protocol::host::Host;
use toolu_runtime::env::Env;
use toolu_runtime::host::cursor::{MARKER, local_dir};
use toolu_runtime::host::roots::Roots;

/// What is at a plugin's directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum State {
  /// Nothing.
  Absent,
  /// A directory toolu installed, at this version (`None` when its manifest says none).
  Ours(Option<String>),
  /// A directory toolu did not install.
  Foreign,
}

/// Cursor's local plugins directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Store {
  /// `<cursor config>/plugins/local`.
  pub(crate) dir: PathBuf,
}

/// The `version` of a plugin directory's Claude-format manifest.
pub(crate) fn version_of(plugin: &Path) -> Option<String> {
  let text = std::fs::read_to_string(plugin.join(".claude-plugin/plugin.json")).ok()?;
  let json: Value = serde_json::from_str(&text).ok()?;
  json.get("version")?.as_str().map(str::to_owned)
}

impl Store {
  /// The store under the Cursor config of `env`.
  pub(crate) fn locate(env: &Env) -> Store {
    Store {
      dir: local_dir(&Roots::new(env.clone(), Some(Host::Cursor))),
    }
  }

  /// Where plugin `name` lives.
  pub(crate) fn path(&self, name: &str) -> PathBuf {
    self.dir.join(name)
  }

  /// What is at `name`.
  pub(crate) fn state(&self, name: &str) -> State {
    let dir = self.path(name);
    if !dir.is_dir() {
      return State::Absent;
    }
    if dir.join(MARKER).is_file() {
      State::Ours(version_of(&dir))
    } else {
      State::Foreign
    }
  }

  /// The names of the plugins toolu installed, sorted.
  pub(crate) fn ours(&self) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(&self.dir) else {
      return Vec::new();
    };
    let mut names: Vec<String> = entries
      .filter_map(Result::ok)
      .filter(|entry| entry.path().join(MARKER).is_file())
      .map(|entry| entry.file_name().to_string_lossy().into_owned())
      .collect();
    names.sort();
    names
  }

  /// Copies `source` to `name`, replacing a plugin of toolu's own: the copy is
  /// finished beside the target and renamed into place.
  pub(crate) fn put(&self, source: &Path, name: &str, tag: &str) -> Result<(), String> {
    let staged = self.dir.join(format!(".{name}.toolu-staging"));
    let io = |what: &str, err: std::io::Error| format!("{what} {}: {err}", staged.display());
    if staged.exists() {
      std::fs::remove_dir_all(&staged).map_err(|err| io("cannot clear", err))?;
    }
    copy_tree(source, &staged).map_err(|err| io("cannot copy into", err))?;
    let marker = json!({"tag": tag, "version": version_of(source)});
    std::fs::write(staged.join(MARKER), format!("{marker}\n"))
      .map_err(|err| io("cannot mark", err))?;
    let target = self.path(name);
    if target.exists() {
      std::fs::remove_dir_all(&target)
        .map_err(|err| format!("cannot replace {}: {err}", target.display()))?;
    }
    std::fs::rename(&staged, &target)
      .map_err(|err| format!("cannot place {}: {err}", target.display()))
  }

  /// Deletes toolu's own copy of `name`.
  pub(crate) fn take(&self, name: &str) -> Result<(), String> {
    let target = self.path(name);
    std::fs::remove_dir_all(&target)
      .map_err(|err| format!("cannot remove {}: {err}", target.display()))
  }
}

/// Copies a directory tree; symlinks and `.git` are left out, so a plugin
/// directory stays self-contained.
fn copy_tree(from: &Path, to: &Path) -> std::io::Result<()> {
  std::fs::create_dir_all(to)?;
  for entry in std::fs::read_dir(from)? {
    let entry = entry?;
    let kind = entry.file_type()?;
    let target = to.join(entry.file_name());
    if kind.is_symlink() || entry.file_name() == ".git" {
      continue;
    }
    if kind.is_dir() {
      copy_tree(&entry.path(), &target)?;
    } else {
      std::fs::copy(entry.path(), &target)?;
    }
  }
  Ok(())
}

#[cfg(test)]
#[path = "tests/store_test.rs"]
mod tests;
