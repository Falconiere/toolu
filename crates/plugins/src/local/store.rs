//! The directory a local-directory host loads plugins from, and the plugins toolu
//! put in it. A plugin directory toolu installed carries a marker file; one
//! without it is the user's and is never touched. An `OpenCode` copy also gets the
//! generated `server.js` shim, written before the copy is renamed into place.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use toolu_protocol::host::Host;
use toolu_protocol::launcher::opencode::{FILE, hooks_of, shim};
use toolu_runtime::env::Env;
use toolu_runtime::host::cursor::{MARKER, local_dir};
use toolu_runtime::host::opencode::plugins_dir;
use toolu_runtime::host::roots::Roots;

use crate::host::Target;

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

/// A host's local plugins directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Store {
  /// The directory the host searches for local plugins.
  pub(crate) dir: PathBuf,
  target: Target,
}

/// The `version` of a plugin directory's Claude-format manifest.
pub(crate) fn version_of(plugin: &Path) -> Option<String> {
  let text = std::fs::read_to_string(plugin.join(".claude-plugin/plugin.json")).ok()?;
  let json: Value = serde_json::from_str(&text).ok()?;
  json.get("version")?.as_str().map(str::to_owned)
}

/// The `OpenCode` shim for the plugin copied from `source` as `name`: the hooks its
/// `hooks.json` needs, and its skills and commands when it has any.
pub(crate) fn opencode_shim(source: &Path, name: &str) -> Result<String, String> {
  let hooks = read_hooks(source, name)?;
  let catalog = ["skills", "commands"]
    .iter()
    .any(|dir| source.join(dir).is_dir());
  shim(name, &hooks_of(&hooks), catalog)
}

/// Read a plugin's shared hook inventory; an absent file means it has no hooks.
pub(super) fn read_hooks(source: &Path, name: &str) -> Result<Value, String> {
  match std::fs::read_to_string(source.join("hooks/hooks.json")) {
    Ok(text) => serde_json::from_str(&text)
      .map_err(|err| format!("{name}: hooks/hooks.json is not JSON: {err}")),
    Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Value::Null),
    Err(err) => Err(format!("{name}: cannot read hooks/hooks.json: {err}")),
  }
}

impl Store {
  /// The store of `target` under the config of `env`.
  pub(crate) fn locate(env: &Env, target: Target) -> Store {
    let roots = |host| Roots::new(env.clone(), Some(host));
    let dir = match target {
      Target::Opencode => plugins_dir(&roots(Host::Opencode)),
      Target::Qwen => roots(Host::Qwen).config_root().join("extensions"),
      Target::Claude | Target::Codex | Target::Grok | Target::Muse | Target::Cursor => {
        local_dir(&roots(Host::Cursor))
      }
    };
    Store { dir, target }
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
    let target = self.path(name);
    let staged = self.dir.join(format!(".{name}.toolu-staging"));
    let io = |what: &str, err: std::io::Error| format!("{what} {}: {err}", staged.display());
    if staged.exists() {
      std::fs::remove_dir_all(&staged).map_err(|err| io("cannot clear", err))?;
    }
    copy_tree(source, &staged).map_err(|err| io("cannot copy into", err))?;
    if self.target == Target::Opencode {
      let text = opencode_shim(source, name)?;
      std::fs::write(staged.join(FILE), text).map_err(|err| io("cannot write the shim in", err))?;
    } else if self.target == Target::Qwen {
      super::qwen::write(source, &staged, &target, name)?;
    }
    let marker = json!({"tag": tag, "version": version_of(source)});
    std::fs::write(staged.join(MARKER), format!("{marker}\n"))
      .map_err(|err| io("cannot mark", err))?;
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
