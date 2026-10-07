//! A worktree's resource binding (`packages/toolu-core/src/resources/binding.ts`):
//! `<git dir>/toolu-resource.json` names the resource home, the owner key and
//! state directory, and the worktree it belongs to. A ledger check run in a
//! bound worktree runs under a machine-wide job lease, whatever environment
//! the command inherited.

use std::path::{Path, PathBuf};

use toolu_runtime::json::is_js_space;
use toolu_runtime::json::ordered::Ordered;

use super::lock::{read_json_file, write_json_atomic};
use crate::ledger::jq::{number, string};
use crate::ledger::parse::resolve;

/// A worktree's binding to a resource home.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceBinding {
  /// The resource home, an absolute path.
  pub root: PathBuf,
  /// The owner's key; each job adds a unique suffix.
  pub key: String,
  /// The owning epic's state directory.
  pub state_dir: String,
  /// The bound worktree.
  pub worktree: PathBuf,
}

/// The git dir and worktree that hold `cwd`, walking up from its real path.
fn git_dir(cwd: &Path) -> Result<Option<(PathBuf, PathBuf)>, String> {
  let mut path = std::fs::canonicalize(cwd).map_err(|err| format!("{}: {err}", cwd.display()))?;
  loop {
    let marker = path.join(".git");
    if let Ok(meta) = std::fs::metadata(&marker) {
      if meta.is_dir() {
        return Ok(Some((marker, path)));
      }
      let text =
        std::fs::read_to_string(&marker).map_err(|err| format!("{}: {err}", marker.display()))?;
      let target = gitfile_target(&text)
        .ok_or_else(|| format!("invalid git worktree marker: {}", marker.display()))?;
      let git = resolve(&path, target.trim_matches(is_js_space));
      return Ok(Some((git, path)));
    }
    if !path.pop() {
      return Ok(None);
    }
  }
}

/// `/^gitdir: (.+)\s*$/`: the first line's target when only white space follows it.
fn gitfile_target(text: &str) -> Option<&str> {
  let rest = text.strip_prefix("gitdir: ")?;
  let (line, after) = rest.split_once('\n').unwrap_or((rest, ""));
  let target = line.strip_suffix('\r').unwrap_or(line);
  (!line.is_empty() && after.chars().all(is_js_space)).then_some(target)
}

/// `bindWorktree(root, worktree, key, stateDir)`.
///
/// # Errors
/// When `worktree` is not in a git worktree, or the binding cannot be written.
pub fn bind_worktree(
  root: &Path,
  worktree: &Path,
  key: &str,
  state_dir: &str,
) -> Result<(), String> {
  let (git, worktree) = git_dir(worktree)?.ok_or("cannot bind resources outside a Git worktree")?;
  let binding = Ordered::Object(vec![
    ("version".to_owned(), number(1.0)),
    ("root".to_owned(), string(&root.display().to_string())),
    ("key".to_owned(), string(key)),
    ("stateDir".to_owned(), string(state_dir)),
    (
      "worktree".to_owned(),
      string(&worktree.display().to_string()),
    ),
  ]);
  write_json_atomic(&git.join("toolu-resource.json"), &binding)
}

/// `resourceBinding(cwd)`: the binding of the worktree holding `cwd`, if any.
///
/// # Errors
/// An unreadable or invalid binding, or a `cwd` that does not exist.
pub fn resource_binding(cwd: &Path) -> Result<Option<ResourceBinding>, String> {
  let Some((git, worktree)) = git_dir(cwd)? else {
    return Ok(None);
  };
  let binding = read_json_file(&git.join("toolu-resource.json"), Ordered::Null)?;
  if matches!(binding, Ordered::Null) {
    return Ok(None);
  }
  let field = |key: &str| match binding.get(key) {
    Some(Ordered::String(value)) => Some(value.clone()),
    _ => None,
  };
  let version_one =
    matches!(binding.get("version"), Some(Ordered::Number(n)) if n.as_f64() == Some(1.0));
  let worktree_text = worktree.display().to_string();
  match (
    field("root"),
    field("key"),
    field("stateDir"),
    field("worktree"),
  ) {
    (Some(root), Some(key), Some(state_dir), Some(bound))
      if version_one && root.starts_with('/') && bound == worktree_text =>
    {
      Ok(Some(ResourceBinding {
        root: PathBuf::from(root),
        key,
        state_dir,
        worktree,
      }))
    }
    _ => Err("invalid worktree resource binding".to_owned()),
  }
}

#[cfg(test)]
#[path = "tests/binding_test.rs"]
mod tests;
