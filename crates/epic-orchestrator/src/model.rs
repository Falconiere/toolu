//! In-memory engine state. Disk and scripts live outside this module.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::journal::Record;

/// What the engine asks the outside world to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Step {
  /// Append this journal record, then commit it.
  Journal(Record),
  /// Run this effect, then commit its outcome.
  Call {
    key: String,
    action: Action,
    token: String,
  },
  /// Rewrite the status snapshot.
  Status { key: String },
  /// Rewrite the issue `stage`.
  Issue { key: String },
}

/// An effect the scripts or a later issue perform.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Action {
  /// Merge the issue's pull request.
  Merge,
  /// Clean up after a merge.
  Cleanup,
  /// Launch a worker.
  Launch,
  /// Snapshot the worktree.
  Checkpoint,
  /// Ask whether an open action already happened.
  Reconcile,
  /// Send text to the worker.
  Prompt,
}

impl Action {
  /// Script and journal name.
  pub(crate) fn name(self) -> &'static str {
    match self {
      Self::Merge => "merge",
      Self::Cleanup => "cleanup",
      Self::Launch => "launch",
      Self::Checkpoint => "checkpoint",
      Self::Reconcile => "reconcile",
      Self::Prompt => "prompt",
    }
  }
}

/// A judgment `wait` may return.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Attention {
  /// Local sequence.
  pub seq: u64,
  /// Judgment kind.
  pub kind: String,
  /// Issue key.
  pub key: String,
  /// Epic key.
  pub epic: String,
  /// Capped note.
  pub note: String,
  /// Already given to a waiter.
  pub delivered: bool,
}

/// An action waiting to be journaled or run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Pending {
  /// What to run.
  pub action: Action,
  /// Stable token.
  pub token: String,
  /// 0 intent, 1 call, 2 done-record.
  pub phase: u8,
  /// The call reconciles a recovered intent.
  pub recovered: bool,
  /// Armed after this action finishes.
  pub follow: Option<Action>,
}

/// One issue the engine is driving.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Issue {
  /// Issue key.
  pub key: String,
  /// Epic key.
  pub epic: String,
  /// Epic state directory.
  pub state_dir: String,
  /// Worker phase.
  pub phase: String,
  /// TypeScript stage.
  pub stage: String,
  /// Pull request, when reported.
  pub pr: Option<u64>,
  /// Last note.
  pub note: String,
  /// Launch attempts.
  pub launches: u32,
  /// A `STATUS?` was sent for the current phase.
  pub stall_nudged: bool,
  /// When the phase last changed, milliseconds.
  pub phase_at_ms: u64,
  /// When the nudge was sent.
  pub nudge_at_ms: u64,
  /// Worktree to checkpoint.
  pub worktree: Option<String>,
  /// Pane bound by the last snapshot.
  pub pane: Option<String>,
  /// Herdr agent name. Defaults to the issue key.
  pub agent: String,
  /// Host kind for a cooldown. Empty writes no cooldown.
  pub kind: String,
  /// Status stayed `blocked` since the last change.
  pub blocked: bool,
  /// `phase_at_ms` of the last limit scan.
  pub scanned_at: Option<u64>,
  /// Issue keys that still block this one.
  pub blockers: Vec<String>,
  /// The action in progress.
  pub pending: Option<Pending>,
  /// An action deferred to a later issue.
  pub deferred: Option<String>,
}

/// Whether a dependency cycle has been announced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Cycle {
  /// No cycle in the graph.
  Clear,
  /// A cycle is present and not yet a judgment.
  Open,
  /// The judgment was raised.
  Sent,
}

/// The engine's memory of every registered issue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct World {
  /// Issues by key.
  pub issues: BTreeMap<String, Issue>,
  /// Judgment queue.
  pub attention: Vec<Attention>,
  /// Local counter for tokens and attention.
  pub seq: u64,
  /// Pause every epic.
  pub paused_all: bool,
  /// Stop before starting another effect.
  pub draining: bool,
  /// Paused epic keys.
  pub paused: BTreeSet<String>,
  /// Clock, milliseconds.
  pub now_ms: u64,
  /// Stall limit for ordinary phases.
  pub stall_ms: u64,
  /// Stall limit while the phase is `babysit`.
  pub babysit_stall_ms: u64,
  /// When the next checkpoint wave may start.
  pub checkpoint_at_ms: u64,
  /// Checkpoint period.
  pub checkpoint_every_ms: u64,
  /// Failed herdr probes in a row.
  pub herdr_failures: u32,
  /// Do not probe before this time.
  pub herdr_retry_at_ms: u64,
  /// Launches in flight at once.
  pub max_parallel: u32,
  /// Dependency-cycle announcement.
  pub cycle: Cycle,
  /// Worktrees already snapshotted in this wave.
  pub checkpointed: BTreeSet<String>,
  /// Steps produced by ticks and events.
  pub outbox: VecDeque<Step>,
}

/// A worker report.
#[derive(Debug, Clone)]
pub(crate) struct Report {
  /// Issue key.
  pub key: String,
  /// Epic key.
  pub epic: String,
  /// State directory.
  pub state_dir: String,
  /// Phase name.
  pub phase: String,
  /// Pull request.
  pub pr: Option<u64>,
  /// Note.
  pub note: String,
}

const STALL_MS: u64 = 45 * 60 * 1000;
const BABYSIT_STALL_MS: u64 = 120 * 60 * 1000;
/// Checkpoint period. Tests and the watcher read this.
pub(crate) const CHECKPOINT_MS: u64 = 15 * 60 * 1000;

impl World {
  /// An empty engine at `now_ms`.
  pub(crate) fn new(now_ms: u64) -> Self {
    Self {
      issues: BTreeMap::new(),
      attention: Vec::new(),
      seq: 0,
      paused_all: false,
      draining: false,
      paused: BTreeSet::new(),
      now_ms,
      stall_ms: STALL_MS,
      babysit_stall_ms: BABYSIT_STALL_MS,
      checkpoint_at_ms: now_ms.saturating_add(CHECKPOINT_MS),
      checkpoint_every_ms: CHECKPOINT_MS,
      herdr_failures: 0,
      herdr_retry_at_ms: 0,
      max_parallel: 1,
      cycle: Cycle::Clear,
      checkpointed: BTreeSet::new(),
      outbox: VecDeque::new(),
    }
  }
}

impl Issue {
  pub(crate) fn blank(key: &str, epic: &str, state_dir: &str, now_ms: u64) -> Self {
    Self {
      key: key.to_owned(),
      epic: epic.to_owned(),
      state_dir: state_dir.to_owned(),
      phase: String::new(),
      stage: String::new(),
      pr: None,
      note: String::new(),
      launches: 0,
      stall_nudged: false,
      phase_at_ms: now_ms,
      nudge_at_ms: 0,
      worktree: None,
      pane: None,
      agent: key.to_owned(),
      kind: String::new(),
      blocked: false,
      scanned_at: None,
      blockers: Vec::new(),
      pending: None,
      deferred: None,
    }
  }
}

pub(crate) fn fresh(action: Action, token: String) -> Pending {
  Pending {
    action,
    token,
    phase: 0,
    recovered: false,
    follow: None,
  }
}

#[cfg(test)]
#[path = "tests/model_test.rs"]
mod tests;
