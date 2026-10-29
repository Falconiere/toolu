//! Worktree operations: plain `git worktree` plus one tmux session each.

use std::path::Path;

use serde_json::{Value, json};
use toolu_runtime::process::{Spec, run};

use super::{TIMEOUT, Tmux, TmuxError};

/// A worktree row, as `worktree.list` returns it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeRow {
  /// The branch checked out, or empty for a detached worktree.
  pub branch: String,
  /// The worktree path.
  pub path: String,
  /// The tmux session name, when one is open for this worktree.
  pub open_workspace_id: Option<String>,
}

/// A created or opened worktree, as `worktree.create` / `worktree.open` return it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Worktree {
  /// The worktree path.
  pub path: String,
  /// The tmux session name (the "workspace id").
  pub workspace_id: String,
  /// The first pane id (the "root pane").
  pub pane_id: String,
}

impl Worktree {
  /// The herdr `result` object for a create/open reply.
  #[must_use]
  pub fn to_value(&self) -> Value {
    json!({
      "worktree": {"path": self.path},
      "workspace": {"workspace_id": self.workspace_id},
      "root_pane": {"pane_id": self.pane_id},
    })
  }
}

impl Tmux {
  /// Create a worktree for `branch` under `cwd` and open a session for it.
  ///
  /// # Errors
  /// git or tmux fails.
  pub fn create_worktree(
    &self,
    cwd: &Path,
    branch: &str,
    base: &str,
    label: &str,
  ) -> Result<Worktree, TmuxError> {
    let raw = cwd.join("..").join(label);
    self.git(
      cwd,
      &[
        "worktree",
        "add",
        &raw.to_string_lossy(),
        "-b",
        branch,
        base,
      ],
    )?;
    let path =
      std::fs::canonicalize(&raw).map_err(|err| TmuxError::new("git", format!("{err:?}")))?;
    self.open_session_at(&path, label)
  }

  /// Open a session for an existing worktree at `path`.
  ///
  /// # Errors
  /// The worktree is missing or tmux fails.
  pub fn open_worktree(&self, path: &Path, label: &str) -> Result<Worktree, TmuxError> {
    let path = std::fs::canonicalize(path).map_err(|err| {
      TmuxError::new(
        "worktree_not_found",
        format!("no worktree at {}: {err}", path.display()),
      )
    })?;
    self.open_session_at(&path, label)
  }

  /// A new session for `label` at the already-canonical `path`, as a [`Worktree`].
  fn open_session_at(&self, path: &Path, label: &str) -> Result<Worktree, TmuxError> {
    let session = session_name_for(label);
    self.create_session(&session, path)?;
    let pane_id = self.first_pane(&session)?;
    Ok(Worktree {
      path: path.to_string_lossy().into_owned(),
      workspace_id: session,
      pane_id,
    })
  }

  /// Remove the session for `workspace_id` and its worktree.
  ///
  /// # Errors
  /// git or tmux fails.
  pub fn remove_worktree(
    &self,
    cwd: &Path,
    workspace_id: &str,
    force: bool,
  ) -> Result<(), TmuxError> {
    let path = self.session_path(workspace_id)?;
    self.kill_session(workspace_id)?;
    let mut args = vec!["worktree", "remove"];
    if force {
      args.push("--force");
    }
    args.push(&path);
    self.git(cwd, &args).map(|_| ())
  }

  /// The worktrees of the repository at `cwd`, with their open sessions.
  ///
  /// # Errors
  /// git or tmux fails.
  pub fn list_worktrees(&self, cwd: &Path) -> Result<Vec<WorktreeRow>, TmuxError> {
    let text = self.git(cwd, &["worktree", "list", "--porcelain"])?;
    Ok(
      text
        .split("\n\n")
        .filter_map(|block| self.parse_worktree_block(block))
        .collect(),
    )
  }

  /// One `worktree list --porcelain` entry, blank lines stripped by the caller.
  fn parse_worktree_block(&self, block: &str) -> Option<WorktreeRow> {
    let path = block
      .lines()
      .find_map(|line| line.strip_prefix("worktree "))?;
    let branch = block
      .lines()
      .find_map(|line| line.strip_prefix("branch "))
      .map_or("", |rest| rest.trim_start_matches("refs/heads/"));
    Some(WorktreeRow {
      branch: branch.to_owned(),
      path: path.to_owned(),
      open_workspace_id: self.session_for_path(path),
    })
  }

  /// The session whose start directory is `path`, when one exists.
  fn session_for_path(&self, path: &str) -> Option<String> {
    let text = self
      .run(&["list-sessions", "-F", "#{session_name}@#{session_path}"])
      .ok()?;
    text.lines().find_map(|line| {
      let (name, dir) = line.split_once('@')?;
      (dir == path).then(|| name.to_owned())
    })
  }

  /// The start directory of session `name`.
  fn session_path(&self, name: &str) -> Result<String, TmuxError> {
    let text = self.run(&["list-sessions", "-F", "#{session_name}@#{session_path}"])?;
    text
      .lines()
      .find_map(|line| {
        let (session, dir) = line.split_once('@')?;
        (session == name).then(|| dir.to_owned())
      })
      .ok_or_else(|| TmuxError::new("workspace_not_found", format!("no session {name}")))
  }

  /// Run `git -C cwd args…` and return its trimmed stdout.
  fn git(&self, cwd: &Path, args: &[&str]) -> Result<String, TmuxError> {
    let mut command = vec![
      "git".to_owned(),
      "-C".to_owned(),
      cwd.to_string_lossy().into_owned(),
    ];
    command.extend(args.iter().map(|arg| (*arg).to_owned()));
    let mut spec = Spec::new(command);
    spec.env = Some(self.env.clone());
    spec.timeout = TIMEOUT;
    let output = run(&spec).map_err(|err| TmuxError::new("git", format!("{err:?}")))?;
    if output.exit_code != 0 {
      return Err(TmuxError::new("git", output.stderr.trim().to_owned()));
    }
    Ok(output.stdout.trim().to_owned())
  }
}

/// A tmux session name for a worktree label, safe for tmux (no `.` or `:`).
fn session_name_for(label: &str) -> String {
  let safe: String = label
    .chars()
    .map(|ch| {
      if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
        ch
      } else {
        '-'
      }
    })
    .collect();
  format!("toolu-{safe}")
}

#[cfg(test)]
#[path = "tests/worktree_test.rs"]
mod tests;
