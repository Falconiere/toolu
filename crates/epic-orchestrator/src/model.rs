//! In-memory engine state. Disk and scripts live outside this module.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::journal::Record;
use crate::watch::Watch;
use toolu_github::RateLimit;

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
  /// Send `text` to the issue's worker; when it cannot be delivered, raise a
  /// `fallback` attention item if one is named.
  Prompt {
    key: String,
    text: String,
    fallback: Option<String>,
  },
  /// Announce the attention item `token` to the notifier.
  Notify { token: String, reminder: bool },
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
  /// Rebuild an epic's graph from its tracker (epic-level).
  Refresh,
  /// Post the summary and close a complete epic (epic-level).
  Close,
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
      Self::Refresh => "refresh",
      Self::Close => "close",
    }
  }
}

/// Where a live attention item stands with its human.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Ack {
  /// Nobody has answered or acknowledged it.
  Open,
  /// `toolu epic ack` saw it; reminders stop.
  Acknowledged,
  /// `toolu epic answer` reached the worker.
  Answered,
}

impl Ack {
  /// Document and journal name.
  pub(crate) fn name(self) -> &'static str {
    match self {
      Self::Open => "open",
      Self::Acknowledged => "acknowledged",
      Self::Answered => "answered",
    }
  }
}

/// A judgment `wait` may return. It stays queued until a report closes it.
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
  /// Stable item token, `a<raised_ms>-<seq>`.
  pub token: String,
  /// When it was raised, milliseconds.
  pub raised_ms: u64,
  /// Open, acknowledged or answered.
  pub state: Ack,
  /// When it was last announced, milliseconds.
  pub announced_ms: u64,
  /// Reminders sent so far.
  pub reminders: u32,
  /// Redacted screen lines of a blocked agent, for `wait` only.
  pub pending: Option<String>,
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
  /// A launch that routes again and replaces the agent (after a host limit).
  pub reroute: bool,
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
  /// Repository that owns this issue and its pull request.
  pub repo: String,
  /// Base branch reported by the last pull request probe.
  pub base: String,
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
  /// herdr's last agent status: `idle`, `working`, `blocked`, `done` or `unknown`.
  pub agent_status: String,
  /// When `agent_status` last changed, milliseconds; 0 before any change.
  pub agent_status_at_ms: u64,
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
  /// A coupling hold: `{kind, with, score, source, at}`; ends only on release.
  pub hold: Option<serde_json::Value>,
  /// Released from a hold: the next launch skips the coupling check.
  pub coupling_released: bool,
  /// Launch attempts that waited in a row (no host, capacity or budget).
  pub wait_streak: u32,
  /// A waiting launch is not tried again before this time.
  pub retry_at_ms: u64,
  /// A relaunch of this running issue that waited: `reroute` (a host limit
  /// moves it) or `lost` (its agent is gone). Refill retries it; the record
  /// keeps it across a restart.
  pub relaunch: Option<String>,
  /// What the engine saw of the tracker item after the merge.
  pub tracker_seen: Seen,
  /// The issue's status in the epic graph (`done` once the tracker closed
  /// it, perhaps before this run); refill never launches a `done` issue.
  pub graph_status: String,
  /// Its place in the merge queue and its rebase rounds.
  pub queue: crate::queue_fields::QueueState,
  /// The last message sent to the worker with no attention item open.
  pub direction: Option<Direction>,
}

/// A message sent to a worker with `toolu epic answer` while nothing was open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Direction {
  /// The message.
  pub text: String,
  /// Who sent it.
  pub by: String,
  /// When it was delivered, milliseconds.
  pub at_ms: u64,
}

/// What the engine saw of a merged issue's tracker item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Seen {
  /// Not seen closed since the merge.
  Open,
  /// Seen closed after the merge (`tracker_closed_at` in the record).
  Closed,
  /// Seen open again; its one `reopened` judgment was raised
  /// (`reopened_raised` in the record).
  Reopened,
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
  /// Registered epic references by epic key.
  pub epic_refs: BTreeMap<String, String>,
  /// GitHub checks and their saved deadlines.
  pub watches: BTreeMap<String, Watch>,
  /// All GitHub requests pause until this deadline after `retry-after`.
  pub github_hold_until_ms: u64,
  /// Latest REST primary budget from a GitHub response.
  pub rest_rate: Option<RateLimit>,
  /// Latest GraphQL primary points remaining.
  pub graphql_remaining: Option<u64>,
  /// GraphQL primary reset in Unix seconds.
  pub graphql_reset_at: Option<u64>,
  /// REST primary points recorded by this engine.
  pub rest_points: u64,
  /// GraphQL points recorded by babysit ticks.
  pub graphql_points: u64,
  /// Judgment queue.
  pub attention: Vec<Attention>,
  /// First reminder delay for an open attention item.
  pub remind_ms: u64,
  /// Local counter for tokens and attention.
  pub seq: u64,
  /// Pause every epic.
  pub paused_all: bool,
  /// Stop before starting another effect.
  pub draining: bool,
  /// The effect thread runs a call; no other call starts until it returns.
  pub effect_busy: bool,
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
  /// Launches in flight at once, for an issue whose epic has no own limit.
  pub max_parallel: u32,
  /// Per registered epic: its graph, limit and current launch batch.
  pub epics: BTreeMap<String, crate::epics::Epic>,
  /// GitHub core and GraphQL floors under which launches and merges wait.
  pub floors: (u64, u64),
  /// Minutes a host cools down after a usage limit.
  pub cooldown_min: u64,
  /// Dependency-cycle announcement.
  pub cycle: Cycle,
  /// Worktrees already snapshotted in this wave.
  pub checkpointed: BTreeSet<String>,
  /// Steps produced by ticks and events.
  pub outbox: VecDeque<Step>,
  /// Worker prompts waiting out their debounce window, by issue key.
  pub prompt_queue: BTreeMap<String, QueuedPrompt>,
  /// Last relay per recipient, bounding worker ping-pong. Relays bypass
  /// `prompt_queue` and go out at once.
  pub relayed_at_ms: BTreeMap<String, u64>,
  /// When the engine last journaled its resource use (Unix ms).
  pub usage_at_ms: u64,
  /// Id of the committed snapshot. Status JSON is cached for one id.
  pub(crate) generation: u64,
}

/// A worker prompt waiting out its debounce window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct QueuedPrompt {
  /// The engine-signed prompt text.
  pub text: String,
  /// The earliest Unix-millisecond time it may be sent.
  pub due_ms: u64,
  /// The journal name of the flushed prompt (`unblocked`).
  pub name: String,
  /// The attention kind raised when delivery fails, if one is owed.
  pub fallback: Option<String>,
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
/// Default first reminder delay, `epic.attention.remind_after_minutes`.
pub(crate) const REMIND_MS: u64 = 30 * 60 * 1000;
/// Checkpoint period. Tests and the watcher read this.
pub(crate) const CHECKPOINT_MS: u64 = 15 * 60 * 1000;

impl World {
  /// An empty engine at `now_ms`.
  pub(crate) fn new(now_ms: u64) -> Self {
    Self {
      issues: BTreeMap::new(),
      epic_refs: BTreeMap::new(),
      watches: BTreeMap::new(),
      github_hold_until_ms: 0,
      rest_rate: None,
      graphql_remaining: None,
      graphql_reset_at: None,
      rest_points: 0,
      graphql_points: 0,
      attention: Vec::new(),
      remind_ms: REMIND_MS,
      seq: 0,
      paused_all: false,
      draining: false,
      effect_busy: false,
      paused: BTreeSet::new(),
      now_ms,
      stall_ms: STALL_MS,
      babysit_stall_ms: BABYSIT_STALL_MS,
      checkpoint_at_ms: now_ms.saturating_add(CHECKPOINT_MS),
      checkpoint_every_ms: CHECKPOINT_MS,
      herdr_failures: 0,
      herdr_retry_at_ms: 0,
      max_parallel: 1,
      epics: BTreeMap::new(),
      floors: (1_000, 500),
      cooldown_min: 60,
      cycle: Cycle::Clear,
      checkpointed: BTreeSet::new(),
      outbox: VecDeque::new(),
      prompt_queue: BTreeMap::new(),
      relayed_at_ms: BTreeMap::new(),
      usage_at_ms: now_ms,
      generation: 0,
    }
  }
}

impl Issue {
  /// Store herdr's status `word` (any other word is `unknown`); its time
  /// moves only when the word changes.
  pub(crate) fn set_agent_status(&mut self, word: &str, now_ms: u64) {
    let word = agent_status_word(word);
    if self.agent_status != word {
      word.clone_into(&mut self.agent_status);
      self.agent_status_at_ms = now_ms;
    }
  }
}

/// herdr's agent status `word`, or `unknown` for any other word.
pub(crate) fn agent_status_word(word: &str) -> &str {
  match word {
    "idle" | "working" | "blocked" | "done" => word,
    _ => "unknown",
  }
}

/// Start a journaled engine action.
pub(crate) fn fresh(action: Action, token: String) -> Pending {
  Pending {
    action,
    token,
    phase: 0,
    recovered: false,
    follow: None,
    reroute: false,
  }
}

#[cfg(test)]
#[path = "tests/model_test.rs"]
mod tests;
