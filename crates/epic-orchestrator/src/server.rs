//! Apply state-machine steps: journal, snapshots, and scripts.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::time::SystemTime;

use serde_json::json;

use crate::commit::commit_effect;
use crate::disk::{write_status, write_value};
use crate::executor::{Call, Executor, Submitted};
use crate::journal::{self, Record};
use crate::logic::{apply_report, arm_ready, commit_journal, next};
use crate::model::{Action, Report, Step, World};
use crate::paths::Paths;
use crate::schedule::{herdr_ok, note_herdr, on_tick, set_pause};
use crate::snapshot::{load, status_path};
use crate::watch;
use toolu_engine::babysit::BabysitTick;
use toolu_github::Client;

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
  pub(crate) scripts: Option<PathBuf>,
  fault: Fault,
  /// Where effects run.
  pub(crate) executor: Executor,
  /// The call on the effect thread.
  pub(crate) running: Option<Call>,
  /// Calls taken from the outbox while the effect thread was busy.
  pub(crate) parked: VecDeque<Step>,
  /// `GIT_TRACE2_EVENT` for checkpoint git, when a test is counting commands.
  pub git_trace: Option<PathBuf>,
  /// `herdr --session` name. Empty uses the default server.
  pub herdr_session: Option<String>,
  pub(crate) now: SystemTime,
  /// A new pane matched a running worktree, so the subscription should reopen.
  pub resubscribe: bool,
  /// Shared GitHub client, created only when a watch first becomes due.
  pub(crate) github: Option<Client>,
  /// The attention notifier thread, when notifications are on.
  pub(crate) notifier: Option<crate::notify::Notifier>,
  /// The environment effects run with.
  pub(crate) env: toolu_runtime::env::Env,
  /// How in-process effects reach GitHub and the trackers.
  pub(crate) reach: crate::reach::Reach,
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
      executor: Executor::Inline,
      running: None,
      parked: VecDeque::new(),
      git_trace: None,
      herdr_session: None,
      now,
      resubscribe: false,
      github: None,
      notifier: None,
      env: toolu_runtime::env::Env::process(),
      reach: crate::reach::Reach::live(),
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
      // A draining engine starts no parked call; it exits when the running
      // effect returns.
      let parked = if self.running.is_none() && !self.world.draining {
        self.parked.pop_front()
      } else {
        None
      };
      let Some(step) = parked.or_else(|| next(&mut self.world)) else {
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
    watch::sync(&mut self.world);
    for watched in self.world.watches.values_mut() {
      if matches!(&watched.kind, watch::Kind::Pr { key, .. } if key == &report.key) {
        watched.request_immediate();
      }
    }
    self.persist_watch()?;
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
  pub(crate) fn tick(&mut self, babysit: &dyn BabysitTick) -> Result<Stop, String> {
    self.refresh_clock();
    crate::snapshot::adopt_new_epics(&mut self.world, &self.paths)?;
    watch::sync(&mut self.world);
    on_tick(&mut self.world);
    self.github_tick(babysit);
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

  fn apply(&mut self, step: Step) -> Result<Applied, String> {
    match step {
      Step::Journal(record) => self.apply_journal(record),
      Step::Call { key, action, token } => self.apply_call(Call { key, action, token }),
      Step::Status { key } => {
        self.write_status(&key)?;
        Ok(Applied::Continue)
      }
      Step::Issue { key } => {
        self.write_issue(&key)?;
        Ok(Applied::Continue)
      }
      Step::Prompt {
        key,
        text,
        fallback,
      } => {
        let sent = crate::attention_ops::send_prompt(self, &key, &text);
        if let (Err(err), Some(kind)) = (sent, fallback) {
          let note = format!("worker prompt not delivered: {err}");
          crate::logic::push_attention(&mut self.world, &kind, &key, &note);
        }
        Ok(Applied::Continue)
      }
      Step::Notify { token, reminder } => {
        crate::notify::announce(self, &token, reminder);
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

  fn apply_call(&mut self, call: Call) -> Result<Applied, String> {
    if self.running.is_some() {
      // A call taken from the outbox while the thread is busy waits its turn.
      self.parked.push_back(Step::Call {
        key: call.key,
        action: call.action,
        token: call.token,
      });
      return Ok(Applied::Continue);
    }
    if self.fault == Fault::BeforeMerge
      && call.action == Action::Merge
      && !recovered(&self.world, &call.key)
    {
      return Ok(Applied::Fault);
    }
    let job = crate::effects_run::job(self, &call);
    match self.executor.submit(call.clone(), job) {
      Submitted::Done(outcome) => self.finish_call(&call, &outcome),
      Submitted::Sent => {
        self.running = Some(call);
        self.world.effect_busy = true;
        Ok(Applied::Continue)
      }
    }
  }

  /// Commit what the effect thread returned, then run what it unblocked.
  ///
  /// # Errors
  /// A step cannot be written.
  pub(crate) fn effect_done(&mut self, call: &Call, outcome: &str) -> Result<Stop, String> {
    self.running = None;
    self.world.effect_busy = false;
    if self.finish_call(call, outcome)? == Applied::Fault {
      return Ok(Stop::Fault);
    }
    self.pump()
  }

  fn finish_call(&mut self, call: &Call, outcome: &str) -> Result<Applied, String> {
    let merge = call.action == Action::Merge && !recovered(&self.world, &call.key);
    if self.fault == Fault::AfterMerge && merge {
      return Ok(Applied::Fault);
    }
    for step in commit_effect(&mut self.world, &call.key, outcome) {
      if self.apply(step)? == Applied::Fault {
        return Ok(Applied::Fault);
      }
    }
    if self.fault == Fault::BeforeMergeRecord && merge {
      return Ok(Applied::Fault);
    }
    Ok(Applied::Continue)
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
    self
      .world
      .issues
      .get(key)
      .map_or(Ok(()), |issue| crate::disk::write_issue(issue, self.now))
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
      .or_else(|| {
        world
          .epics
          .contains_key(&record.key)
          .then(|| record.key.clone())
      })
      .unwrap_or_default();
  }
  record
}

#[cfg(test)]
#[path = "tests/server_test.rs"]
mod tests;

#[cfg(test)]
#[path = "tests/watcher_test.rs"]
mod watcher_tests;
