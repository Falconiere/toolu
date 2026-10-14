//! `dispatch-fix cleanup`: stop the active fixer, remove the slot worktree and
//! its branch when origin holds its commits, and the round's files.

use std::path::Path;
use std::time::Duration;

use serde_json::{Value, json};

use super::dispatcher::Dispatcher;
use super::herdr::{argv, command, reachable, try_herdr};
use super::worktree::clean_or_fail;
use crate::error::{Failure, Result};

const TIMEOUT: Duration = Duration::from_secs(300);

fn text<'v>(value: &'v Value, key: &str) -> &'v str {
  value.get(key).and_then(Value::as_str).unwrap_or_default()
}

impl Dispatcher<'_> {
  fn stop_active(&self) {
    let groups = self
      .fixer_field("groups")
      .and_then(Value::as_array)
      .cloned()
      .unwrap_or_default();
    let active = groups
      .iter()
      .find(|group| matches!(text(group, "status"), "running" | "launching" | "blocked"));
    if let Some(active) = active.filter(|_| !self.dry) {
      self.stop_fixer(active);
    }
  }

  fn remove_worktree(&mut self, worktree: &Value) -> Result<()> {
    let (path, root) = (text(worktree, "path"), text(worktree, "repoRoot"));
    let remove = argv(&["git", "-C", root, "worktree", "remove", "--force", path]);
    let Some(workspace) = worktree.get("workspaceId").and_then(Value::as_str) else {
      return self
        .git(
          &remove,
          "git_error",
          format!("could not remove the fixer worktree {path}"),
        )
        .map(drop);
    };
    if !self.dry && !reachable(self.env) {
      return Err(Failure::new(
        "herdr_unavailable",
        format!("herdr is not reachable; cannot remove workspace {workspace}"),
      ));
    }
    let herdr_remove = argv(&[
      "herdr",
      "worktree",
      "remove",
      "--workspace",
      workspace,
      "--force",
    ]);
    self.record(&herdr_remove);
    if self.dry {
      return Ok(());
    }
    match try_herdr(self.env, herdr_remove.get(1..).unwrap_or_default(), TIMEOUT) {
      Ok(_) => Ok(()),
      Err(error) => {
        let message = format!(
          "could not remove the fixer worktree {path}: {}",
          error.message
        );
        self.git(&remove, "herdr_error", message).map(drop)
      }
    }
  }

  fn drop_slot_branch(&mut self, worktree: &Value) -> (bool, Option<String>) {
    let (root, branch, pr) = (
      text(worktree, "repoRoot"),
      text(worktree, "branch"),
      text(worktree, "prBranch"),
    );
    let verify = argv(&[
      "git",
      "-C",
      root,
      "rev-parse",
      "--verify",
      "--quiet",
      &format!("refs/heads/{branch}"),
    ]);
    if !self.dry && !self.probe(&verify) {
      return (false, None);
    }
    let ancestor = argv(&[
      "git",
      "-C",
      root,
      "merge-base",
      "--is-ancestor",
      &format!("refs/heads/{branch}"),
      &format!("refs/remotes/origin/{pr}"),
    ]);
    if !self.dry && !self.probe(&ancestor) {
      return (
        false,
        Some(format!(
          "kept {branch}: it holds commits origin/{pr} does not"
        )),
      );
    }
    let drop_branch = argv(&["git", "-C", root, "branch", "--quiet", "-D", branch]);
    self.record(&drop_branch);
    if self.dry || self.probe(&drop_branch) {
      return (true, None);
    }
    (
      false,
      Some(format!(
        "kept {branch}: git could not delete it (still checked out elsewhere?)"
      )),
    )
  }
}

impl Dispatcher<'_> {
  fn clear(&mut self, key: &str) -> Result<()> {
    self.save(|state| {
      if let Value::Object(root) = state {
        root.insert(key.to_owned(), Value::Null);
      }
    })
  }

  /// Removes the recorded worktree and its branch; whether the worktree went,
  /// and whether the branch did with a note when it stays.
  fn retire(&mut self, worktree: &Value) -> Result<(bool, (bool, Option<String>))> {
    let path = text(worktree, "path").to_owned();
    let present = self.dry || Path::new(&path).exists();
    if present {
      if !self.dry {
        clean_or_fail(self, &path)?;
      }
      self.remove_worktree(worktree)?;
    }
    self.refresh(worktree);
    let dropped = self.drop_slot_branch(worktree);
    self.clear("herdrWorktree")?;
    Ok((present, dropped))
  }

  fn refresh(&mut self, worktree: &Value) {
    let root = text(worktree, "repoRoot");
    let prune = argv(&["git", "-C", root, "worktree", "prune"]);
    let fetch = argv(&[
      "git",
      "-C",
      root,
      "fetch",
      "--quiet",
      "origin",
      text(worktree, "prBranch"),
    ]);
    for step in [prune, fetch] {
      self.record(&step);
      if !self.dry {
        command(self.env, &step, TIMEOUT);
      }
    }
  }

  fn remove_round_files(&self) {
    let Some(stem) = self.file.file_name().and_then(|name| name.to_str()) else {
      return;
    };
    let prefix = format!("{}.fixer-", stem.strip_suffix(".json").unwrap_or(stem));
    let dir = self.file.parent().unwrap_or_else(|| Path::new("."));
    let entries = std::fs::read_dir(dir).into_iter().flatten().flatten();
    for entry in entries.filter(|entry| entry.file_name().to_string_lossy().starts_with(&prefix)) {
      let path = entry.path();
      let _removed = if path.is_dir() {
        std::fs::remove_dir_all(&path)
      } else {
        std::fs::remove_file(&path)
      };
    }
  }

  /// Ends the round and removes what it left; the cleanup document.
  ///
  /// # Errors
  /// `worktree_dirty`, `herdr_unavailable` and git failures.
  pub(crate) fn cleanup(&mut self) -> Result<Value> {
    self.stop_active();
    self.clear("fixer")?;
    let worktree = self
      .state
      .get("herdrWorktree")
      .filter(|worktree| worktree.is_object())
      .cloned();
    let (removed, (deleted, note)) = match worktree {
      None => (
        false,
        (false, Some("no herdr worktree recorded".to_owned())),
      ),
      Some(worktree) => self.retire(&worktree)?,
    };
    if self.dry {
      return Ok(json!({"dryRun": true, "commands": self.commands}));
    }
    self.remove_round_files();
    Ok(
      json!({"version": 1, "status": "cleaned", "worktreeRemoved": removed, "branchDeleted": deleted, "note": note}),
    )
  }
}

#[cfg(test)]
#[path = "tests/cleanup_test.rs"]
mod tests;
