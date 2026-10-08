//! Apply state-machine steps: journal, snapshots, and scripts.

use std::path::PathBuf;
use std::time::SystemTime;

use serde_json::json;

use crate::checkpoint::snapshot;
use crate::commit::commit_effect;
use crate::disk::{write_stage, write_status, write_value};
use crate::effects::invoke;
use crate::journal::{self, Record};
use crate::logic::{apply_report, arm_ready, commit_journal, next};
use crate::model::{Action, Report, Step, World};
use crate::paths::Paths;
use crate::schedule::{note_herdr, on_tick, set_pause};
use crate::snapshot::{issue_path, load, status_path};

/// Where a scripted merge is killed so recovery can be tested.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Fault {
  /// Run the effect.
  None,
  /// Exit after the intent line, before the merge script.
  BeforeMerge,
  /// Exit after the merge script, before `commit_effect`.
  AfterMerge,
  /// Exit after the stage becomes `cleaning`, before the done line.
  BeforeMergeRecord,
}

/// Why `pump` returned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Stop {
  /// Nothing left to do.
  Idle,
  /// `Fault` fired. The process exits 75.
  Fault,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Applied {
  Continue,
  Fault,
}

/// One loaded resource root.
pub(crate) struct Engine {
  /// Issues and deadlines.
  pub world: World,
  pub(crate) paths: Paths,
  scripts: Option<PathBuf>,
  fault: Fault,
  /// `GIT_TRACE2_EVENT` for checkpoint git, when a test is counting commands.
  pub git_trace: Option<PathBuf>,
  /// `herdr --session` name. Empty uses the default server.
  pub herdr_session: Option<String>,
  pub(crate) now: SystemTime,
  /// A new pane matched a running worktree, so the subscription should reopen.
  pub resubscribe: bool,
}

impl Engine {
  /// Load snapshots and the journal.
  ///
  /// # Errors
  /// A snapshot or the journal cannot be read.
  pub(crate) fn open(paths: Paths, scripts: Option<PathBuf>, fault: Fault) -> Result<Self, String> {
    let now = SystemTime::now();
    let world = load(&paths, now)?;
    Ok(Self {
      world,
      paths,
      scripts,
      fault,
      git_trace: None,
      herdr_session: None,
      now,
      resubscribe: false,
    })
  }

  /// Write `watch.json`, including the herdr backoff.
  ///
  /// # Errors
  /// The watch file cannot be replaced.
  pub(crate) fn save_watch(&self) -> Result<(), String> {
    self.persist_watch()
  }

  /// Run until idle or a fault.
  ///
  /// # Errors
  /// A step cannot be written, or the machine does not settle.
  pub(crate) fn pump(&mut self) -> Result<Stop, String> {
    for _ in 0..512 {
      let Some(step) = next(&mut self.world) else {
        return Ok(Stop::Idle);
      };
      if self.apply(step)? == Applied::Fault {
        return Ok(Stop::Fault);
      }
    }
    Err("engine did not settle".to_owned())
  }

  /// Apply a worker report without starting the effect it arms.
  ///
  /// # Errors
  /// A snapshot or the journal cannot be written.
  pub(crate) fn report(&mut self, report: &Report) -> Result<(), String> {
    for step in apply_report(&mut self.world, report) {
      let _applied = self.apply(step)?;
    }
    crate::limit::on_failed(self, &report.phase, &report.note, &report.key)
  }

  /// Journal steps sitting in the outbox.
  ///
  /// # Errors
  /// A step cannot be written.
  pub(crate) fn flush(&mut self) -> Result<(), String> {
    while let Some(step) = self.world.outbox.pop_front() {
      let _applied = self.apply(step)?;
    }
    Ok(())
  }

  /// Persist pause for every epic, or one epic.
  ///
  /// # Errors
  /// The pause file cannot be written.
  pub(crate) fn pause(&mut self, epic: Option<&str>) -> Result<(), String> {
    set_pause(&mut self.world, epic, true);
    self.persist_pause()
  }

  /// Clear pause and arm ready issues.
  ///
  /// # Errors
  /// The pause file cannot be written.
  pub(crate) fn resume(&mut self, epic: Option<&str>) -> Result<(), String> {
    set_pause(&mut self.world, epic, false);
    arm_ready(&mut self.world);
    self.persist_pause()
  }

  /// Move the engine clock to wall time. Ticks and socket messages call this.
  pub(crate) fn refresh_clock(&mut self) {
    self.now = SystemTime::now();
    self.world.now_ms = toolu_state::time::epoch_millis(self.now);
  }

  /// Checkpoint wave, then whatever it armed.
  ///
  /// # Errors
  /// A step or the watch file fails.
  pub(crate) fn tick(&mut self) -> Result<Stop, String> {
    self.refresh_clock();
    crate::snapshot::adopt_new_epics(&mut self.world, &self.paths)?;
    on_tick(&mut self.world);
    journal::retain(&self.paths.journal_dir(), 90, self.now)?;
    self.persist_watch()?;
    if !crate::source::prompt_enabled() {
      self.probe_herdr()?;
    }
    self.pump()
  }

  /// Probe herdr unless the backoff window is still open.
  ///
  /// # Errors
  /// The watch file cannot be written. The probe's own failure is stored.
  pub(crate) fn probe_herdr(&mut self) -> Result<(), String> {
    if self.world.now_ms < self.world.herdr_retry_at_ms {
      return Ok(());
    }
    note_herdr(&mut self.world, herdr_ok(self.herdr_session.as_deref()));
    self.persist_watch()
  }
}

impl Engine {
  fn persist_pause(&self) -> Result<(), String> {
    let epics: Vec<&str> = self.world.paused.iter().map(String::as_str).collect();
    write_value(
      &self.paths.pause(),
      &json!({"all": self.world.paused_all, "epics": epics}),
    )
  }

  fn persist_watch(&self) -> Result<(), String> {
    write_value(
      &self.paths.watch(),
      &json!({
        "version": 1,
        "nextCheckpointAt": self.world.checkpoint_at_ms,
        "checkpointQueue": [],
        "nextBudgetAt": self.world.now_ms,
        "budgetAlertReset": 0,
        "budgetHoldUntil": 0,
        "herdrFailures": self.world.herdr_failures,
        "herdrRetryAt": self.world.herdr_retry_at_ms,
      }),
    )
  }

  fn apply(&mut self, step: Step) -> Result<Applied, String> {
    match step {
      Step::Journal(record) => self.apply_journal(record),
      Step::Call { key, action, .. } => self.apply_call(&key, action),
      Step::Status { key } => {
        self.write_status(&key)?;
        Ok(Applied::Continue)
      }
      Step::Issue { key } => {
        self.write_issue(&key)?;
        Ok(Applied::Continue)
      }
    }
  }

  fn apply_journal(&mut self, record: Record) -> Result<Applied, String> {
    let record = stamp(&self.world, record);
    let stored = journal::append(
      &self.paths.journal_dir(),
      &self.paths.journal_lock(),
      &record,
      self.now,
    )?;
    for step in commit_journal(&mut self.world, &stored) {
      if self.apply(step)? == Applied::Fault {
        return Ok(Applied::Fault);
      }
    }
    Ok(Applied::Continue)
  }

  fn apply_call(&mut self, key: &str, action: Action) -> Result<Applied, String> {
    if action == Action::Prompt {
      if self.scripts.is_none() && crate::source::prompt_enabled() {
        let target = self
          .world
          .issues
          .get(key)
          .map_or_else(|| key.to_owned(), |issue| issue.agent.clone());
        let _sent =
          crate::source::ask_status(&self.paths, &toolu_runtime::env::Env::process(), &target);
      } else {
        let _outcome = invoke(self.scripts.as_deref(), action, "{}");
      }
      return Ok(Applied::Continue);
    }
    if self.fault == Fault::BeforeMerge && action == Action::Merge && !recovered(&self.world, key) {
      return Ok(Applied::Fault);
    }
    let outcome = self.perform(key, action);
    if self.fault == Fault::AfterMerge && action == Action::Merge && !recovered(&self.world, key) {
      return Ok(Applied::Fault);
    }
    for step in commit_effect(&mut self.world, key, &outcome) {
      if self.apply(step)? == Applied::Fault {
        return Ok(Applied::Fault);
      }
    }
    if self.fault == Fault::BeforeMergeRecord
      && action == Action::Merge
      && !recovered(&self.world, key)
    {
      return Ok(Applied::Fault);
    }
    Ok(Applied::Continue)
  }

  fn perform(&self, key: &str, action: Action) -> String {
    if action == Action::Checkpoint {
      return self.checkpoint(key);
    }
    let body = json!({
      "key": key,
      "action": action.name(),
      "epic": self.world.issues.get(key).map(|issue| issue.epic.clone()).unwrap_or_default(),
    });
    invoke(self.scripts.as_deref(), action, &body.to_string())
  }

  fn checkpoint(&self, key: &str) -> String {
    let Some(tree) = self
      .world
      .issues
      .get(key)
      .and_then(|issue| issue.worktree.clone())
    else {
      return "applied".to_owned();
    };
    match snapshot(&tree, key, self.git_trace.as_deref()) {
      Ok(()) => "applied".to_owned(),
      Err(_) => "failed".to_owned(),
    }
  }

  fn write_status(&self, key: &str) -> Result<(), String> {
    let Some(issue) = self.world.issues.get(key) else {
      return Ok(());
    };
    write_status(
      &status_path(&issue.state_dir, key),
      &issue.phase,
      issue.pr,
      &issue.note,
      self.now,
    )
  }

  fn write_issue(&self, key: &str) -> Result<(), String> {
    let Some(issue) = self.world.issues.get(key) else {
      return Ok(());
    };
    if issue.stage.is_empty() {
      return Ok(());
    }
    write_stage(&issue_path(&issue.state_dir, key), &issue.stage)
  }
}

fn recovered(world: &World, key: &str) -> bool {
  world
    .issues
    .get(key)
    .and_then(|issue| issue.pending.as_ref())
    .is_some_and(|pending| pending.recovered)
}

fn stamp(world: &World, mut record: Record) -> Record {
  if record.epic.is_empty() {
    record.epic = world
      .issues
      .get(&record.key)
      .map(|issue| issue.epic.clone())
      .unwrap_or_default();
  }
  record
}

fn herdr_ok(session: Option<&str>) -> bool {
  let mut argv = vec!["herdr".to_owned()];
  if let Some(session) = session {
    argv.push("--session".to_owned());
    argv.push(session.to_owned());
  }
  argv.push("agent".to_owned());
  argv.push("list".to_owned());
  let mut spec = toolu_runtime::process::Spec::new(argv);
  spec.timeout = std::time::Duration::from_secs(10);
  toolu_runtime::process::run(&spec).is_ok_and(|output| output.exit_code == 0)
}

#[cfg(test)]
#[path = "tests/server_test.rs"]
mod tests;

#[cfg(test)]
#[path = "tests/watcher_test.rs"]
mod watcher_tests;
