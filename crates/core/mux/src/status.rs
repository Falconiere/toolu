//! A fallback `agent_status` derived from tmux pane state, used when no host
//! hook has reported one. herdr derives richer states by watching the agent
//! CLI directly; this heuristic only sees the pane's foreground process and
//! its recent screen, so it cannot tell an idle agent from one still warming
//! up. It answers `blocked` on a known permission prompt, `done` once the
//! pane's shell is back in the foreground, `working` while another process
//! owns the pane, and `unknown` when the foreground command cannot be read.

/// One of herdr's `agent_status` values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentStatus {
  /// Waiting for input, ready for a prompt.
  Idle,
  /// Running a task.
  Working,
  /// Waiting on a permission or trust prompt.
  Blocked,
  /// The agent process exited.
  Done,
  /// The pane's state could not be read.
  Unknown,
}

impl AgentStatus {
  /// The status word herdr and the engines use.
  #[must_use]
  pub fn as_str(self) -> &'static str {
    match self {
      AgentStatus::Idle => "idle",
      AgentStatus::Working => "working",
      AgentStatus::Blocked => "blocked",
      AgentStatus::Done => "done",
      AgentStatus::Unknown => "unknown",
    }
  }
}

/// Permission and trust prompts shared by the claude, codex, cursor and muse CLIs.
const BLOCKED_MARKERS: &[&str] = &[
  "Do you want to proceed",
  "Do you want to make this edit",
  "(y/n)",
  "trust this folder",
  "trust this workspace",
  "Yes, allow",
  "Press Enter to continue",
];

/// The common shell names a pane returns to once its foreground process exits.
const SHELLS: &[&str] = &["bash", "zsh", "sh", "dash", "fish"];

/// The fallback status for a pane whose foreground command is `foreground`
/// (tmux's `pane_current_command`) and whose recent screen is `screen`.
#[must_use]
pub fn derive(foreground: &str, screen: &str) -> AgentStatus {
  if BLOCKED_MARKERS.iter().any(|marker| screen.contains(marker)) {
    return AgentStatus::Blocked;
  }
  if foreground.is_empty() {
    return AgentStatus::Unknown;
  }
  if SHELLS.contains(&foreground) {
    return AgentStatus::Done;
  }
  AgentStatus::Working
}

#[cfg(test)]
#[path = "tests/status_test.rs"]
mod tests;
