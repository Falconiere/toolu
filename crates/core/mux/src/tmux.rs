//! The plain-tmux driver: sessions, panes and worktrees without herdr.
//!
//! One named tmux session plays herdr's "workspace"; its first pane plays the
//! "root pane" an agent runs in. Worktrees are plain `git worktree` calls, so
//! the driver returns the same `result` objects herdr's socket does, letting
//! the engines switch backends without changing what they parse.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use toolu_runtime::env::Env;
use toolu_runtime::process::{Spec, run};

use crate::status::{self, AgentStatus};

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

  /// Kill this socket's server and every session on it.
  ///
  /// # Errors
  /// No server runs on the socket, or tmux fails.
  pub fn kill_server(&self) -> Result<(), TmuxError> {
    self.run(&["kill-server"]).map(|_| ())
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
}

/// Reading and writing a pane's contents.
impl Tmux {
  /// Send literal text to `pane`, then Enter.
  ///
  /// # Errors
  /// The pane is missing or tmux fails.
  pub fn send_literal(&self, pane: &str, text: &str) -> Result<(), TmuxError> {
    self
      .run(&["send-keys", "-t", pane, "-l", text])
      .map_err(|err| not_found_as_pane(pane, &err))?;
    self
      .run(&["send-keys", "-t", pane, "Enter"])
      .map(|_| ())
      .map_err(|err| not_found_as_pane(pane, &err))
  }

  /// Send named keys to `pane`.
  ///
  /// # Errors
  /// The pane is missing or tmux fails.
  pub fn send_keys(&self, pane: &str, keys: &[&str]) -> Result<(), TmuxError> {
    let mut args = vec!["send-keys", "-t", pane];
    args.extend_from_slice(keys);
    self
      .run(&args)
      .map(|_| ())
      .map_err(|err| not_found_as_pane(pane, &err))
  }

  /// Capture the last `lines` lines of `pane`, joined into one string.
  ///
  /// # Errors
  /// The pane is missing or tmux fails.
  pub fn capture(&self, pane: &str, lines: usize) -> Result<String, TmuxError> {
    let start = format!("-{lines}");
    self
      .run(&["capture-pane", "-p", "-t", pane, "-S", &start, "-J"])
      .map_err(|err| not_found_as_pane(pane, &err))
  }

  /// Show `text` in session `name`'s status line.
  ///
  /// # Errors
  /// The session is missing or tmux fails.
  pub fn display_message(&self, name: &str, text: &str) -> Result<(), TmuxError> {
    self.run(&["display-message", "-t", name, text]).map(|_| ())
  }
}

/// Agent lifecycle operations: start, prompt and the fallback status.
impl Tmux {
  /// Start `argv` in `pane` by typing it into the pane's shell.
  ///
  /// # Errors
  /// `argv` is empty, the pane is missing, or tmux fails.
  pub fn start_agent(&self, pane: &str, argv: &[&str]) -> Result<(), TmuxError> {
    if argv.is_empty() {
      return Err(TmuxError::new("agent_not_ready", "empty command"));
    }
    self.send_literal(pane, &argv.join(" "))
  }

  /// Send `text` to the agent running in `pane`, then Enter.
  ///
  /// # Errors
  /// The pane is missing or tmux fails.
  pub fn prompt(&self, pane: &str, text: &str) -> Result<(), TmuxError> {
    self.send_literal(pane, text)
  }

  /// The fallback [`AgentStatus`] of `pane`: a known permission prompt is
  /// `blocked`, the pane's own shell back in the foreground is `done`,
  /// anything else running is `working`.
  ///
  /// # Errors
  /// The pane is missing or tmux fails.
  pub fn pane_status(&self, pane: &str) -> Result<AgentStatus, TmuxError> {
    let foreground = self.pane_field(pane, "#{pane_current_command}")?;
    let screen = self.capture(pane, 40).unwrap_or_default();
    Ok(status::derive(foreground.trim(), &screen))
  }

  /// Poll `pane`'s status every 200 ms until it is one of `until`.
  ///
  /// # Errors
  /// The pane is missing, tmux fails, or `timeout` passes first.
  pub fn wait(
    &self,
    pane: &str,
    until: &[AgentStatus],
    timeout: Duration,
  ) -> Result<AgentStatus, TmuxError> {
    let deadline = Instant::now() + timeout;
    loop {
      let status = self.pane_status(pane)?;
      if until.contains(&status) {
        return Ok(status);
      }
      if Instant::now() >= deadline {
        return Err(TmuxError::new(
          "timeout",
          format!("pane {pane} did not reach the wanted status in time"),
        ));
      }
      std::thread::sleep(WAIT_POLL.min(deadline.saturating_duration_since(Instant::now())));
    }
  }
}

/// The wait between two status polls.
const WAIT_POLL: Duration = Duration::from_millis(200);

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

/// Worktree operations: plain `git worktree` plus one tmux session each.
mod worktree;
pub use worktree::{Worktree, WorktreeRow};

#[cfg(test)]
#[path = "tests/tmux_test.rs"]
mod tests;
