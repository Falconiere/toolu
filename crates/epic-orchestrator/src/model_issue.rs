//! The blank issue the engine overlays its graph and worker status on.

use crate::model::{Issue, Seen};

impl Issue {
  /// An issue before its registered graph and worker status are overlaid.
  pub(crate) fn blank(key: &str, epic: &str, state_dir: &str, now_ms: u64) -> Self {
    Self {
      key: key.to_owned(),
      epic: epic.to_owned(),
      state_dir: state_dir.to_owned(),
      phase: String::new(),
      stage: String::new(),
      pr: None,
      repo: String::new(),
      base: String::new(),
      note: String::new(),
      launches: 0,
      stall_nudged: false,
      phase_at_ms: now_ms,
      nudge_at_ms: 0,
      worktree: None,
      pane: None,
      agent: key.to_owned(),
      agent_status: "unknown".to_owned(),
      agent_status_at_ms: 0,
      kind: String::new(),
      blocked: false,
      scanned_at: None,
      blockers: Vec::new(),
      pending: None,
      deferred: None,
      hold: None,
      coupling_released: false,
      wait_streak: 0,
      retry_at_ms: 0,
      relaunch: None,
      tracker_seen: Seen::Open,
      graph_status: String::new(),
      queue: crate::queue_fields::QueueState::default(),
      direction: None,
    }
  }
}
