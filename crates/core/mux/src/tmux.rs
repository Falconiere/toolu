//! The plain-tmux driver: sessions, panes and worktrees without herdr.
//!
//! One named tmux session plays herdr's "workspace"; its first pane plays the
//! "root pane" an agent runs in. Worktrees are plain `git worktree` calls, so
//! the driver returns the same `result` objects herdr's socket does, letting
//! the engines switch backends without changing what they parse.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::{Value, json};
use toolu_runtime::env::Env;
use toolu_runtime::process::{Spec, run};

/// A tmux failure carrying a herdr-compatible error code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TmuxError {
  /// The herdr-compatible code the caller branches on.
  pub code: &'static str,
  /// What failed.
  pub message: String,
}

impl TmuxError {
  fn new(code: &'static str, message: impl Into<String>) -> TmuxError {
    TmuxError {
      code,
      message: message.into(),
    }
  }
}

/// A tmux client bound to one socket.
pub struct Tmux {
  /// The `-S` socket, or `None` for the default socket.
  socket: Option<PathBuf>,
  env: Env,
}

/// The default timeout for a tmux command.
const TIMEOUT: Duration = Duration::from_secs(30);

impl Tmux {
  /// A client for `env` on `socket` (`None` is the default socket).
  #[must_use]
  pub fn new(env: &Env, socket: Option<PathBuf>) -> Tmux {
    Tmux {
      socket,
      env: env.clone(),
    }
  }

  /// Run `tmux [ -S socket ] args…` and return its trimmed stdout.
  ///
  /// # Errors
  /// The command fails to start, times out, or exits non-zero.
  fn run(&self, args: &[&str]) -> Result<String, TmuxError> {
    let mut command = vec!["tmux".to_owned()];
    if let Some(socket) = &self.socket {
      command.push("-S".to_owned());
      command.push(socket.to_string_lossy().into_owned());
    }
    command.extend(args.iter().map(|arg| (*arg).to_owned()));
    let mut spec = Spec::new(command);
    spec.env = Some(self.env.clone());
    spec.timeout = TIMEOUT;
    let output = run(&spec).map_err(|err| TmuxError::new("tmux", format!("{err:?}")))?;
    if output.timed_out {
      return Err(TmuxError::new("timeout", "tmux command timed out"));
    }
    if output.exit_code != 0 {
      return Err(TmuxError::new("tmux", output.stderr.trim().to_owned()));
    }
    Ok(output.stdout.trim().to_owned())
  }

  /// Whether a session named `name` exists.
  #[must_use]
  pub fn session_exists(&self, name: &str) -> bool {
    self.run(&["has-session", "-t", name]).is_ok()
  }

  /// Create a detached session `name` whose first pane starts in `cwd`.
  ///
  /// # Errors
  /// The session already exists or tmux fails.
  pub fn create_session(&self, name: &str, cwd: &Path) -> Result<(), TmuxError> {
    if self.session_exists(name) {
      return Err(TmuxError::new(
        "workspace_exists",
        format!("session {name} exists"),
      ));
    }
    self
      .run(&[
        "new-session",
        "-d",
        "-s",
        name,
        "-c",
        &cwd.to_string_lossy(),
      ])
      .map(|_| ())
  }

  /// Kill session `name`, ignoring a missing session.
  ///
  /// # Errors
  /// tmux fails for a reason other than the session being absent.
  pub fn kill_session(&self, name: &str) -> Result<(), TmuxError> {
    if !self.session_exists(name) {
      return Ok(());
    }
    self.run(&["kill-session", "-t", name]).map(|_| ())
  }

  /// Every session name, one per line.
  ///
  /// # Errors
  /// tmux fails.
  pub fn list_sessions(&self) -> Result<Vec<String>, TmuxError> {
    let text = self.run(&["list-sessions", "-F", "#{session_name}"])?;
    Ok(text.lines().map(str::to_owned).collect())
  }

  /// The first pane id of session `name`.
  ///
  /// # Errors
  /// The session is missing or tmux fails.
  pub fn first_pane(&self, name: &str) -> Result<String, TmuxError> {
    self.pane_field(name, "#{pane_id}")
  }

  /// The process id of the shell running in `pane`.
  ///
  /// # Errors
  /// The pane is missing or tmux fails.
  pub fn pane_pid(&self, pane: &str) -> Result<u32, TmuxError> {
    let text = self.pane_field(pane, "#{pane_pid}")?;
    text
      .parse::<u32>()
      .map_err(|_err| TmuxError::new("pane_not_found", format!("no pid for pane {pane}")))
  }

  /// The first `format` field tmux reports for `target`, a session or a pane.
  fn pane_field(&self, target: &str, format: &str) -> Result<String, TmuxError> {
    let text = self
      .run(&["list-panes", "-t", target, "-F", format])
      .map_err(|err| not_found_as_pane(target, &err))?;
    text
      .lines()
      .next()
      .map(str::to_owned)
      .ok_or_else(|| TmuxError::new("pane_not_found", format!("no pane for {target}")))
  }

  /// Send literal text to `pane`, then Enter.
  ///
  /// # Errors
  /// The pane is missing or tmux fails.
  pub fn send_literal(&self, pane: &str, text: &str) -> Result<(), TmuxError> {
    self.run(&["send-keys", "-t", pane, "-l", text])?;
    self.run(&["send-keys", "-t", pane, "Enter"]).map(|_| ())
  }

  /// Send named keys to `pane`.
  ///
  /// # Errors
  /// The pane is missing or tmux fails.
  pub fn send_keys(&self, pane: &str, keys: &[&str]) -> Result<(), TmuxError> {
    let mut args = vec!["send-keys", "-t", pane];
    args.extend_from_slice(keys);
    self.run(&args).map(|_| ())
  }

  /// Capture the last `lines` lines of `pane`, joined into one string.
  ///
  /// # Errors
  /// The pane is missing or tmux fails.
  pub fn capture(&self, pane: &str, lines: usize) -> Result<String, TmuxError> {
    let start = format!("-{lines}");
    self.run(&["capture-pane", "-p", "-t", pane, "-S", &start, "-J"])
  }

  /// Show `text` in session `name`'s status line.
  ///
  /// # Errors
  /// The session is missing or tmux fails.
  pub fn display_message(&self, name: &str, text: &str) -> Result<(), TmuxError> {
    self.run(&["display-message", "-t", name, text]).map(|_| ())
  }
}

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

/// The worktree operations: plain `git worktree` plus one tmux session each.
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

/// Remap tmux's own "can't find session/pane/window" failure to `pane_not_found`.
fn not_found_as_pane(target: &str, err: &TmuxError) -> TmuxError {
  if err.code == "tmux" && err.message.contains("can't find") {
    return TmuxError::new(
      "pane_not_found",
      format!("no pane for {target}: {}", err.message),
    );
  }
  err.clone()
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
#[path = "tests/tmux_test.rs"]
mod tests;
