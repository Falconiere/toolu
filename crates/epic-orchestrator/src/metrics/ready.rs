//! First `ready` to merged, split by where the time went: waiting in the
//! queue, a rebase, CI after a step, the merge step itself, or rework after
//! the issue left the queue. A queue step is credited when its outcome is
//! known; a merged step journals no outcome.

use std::collections::{BTreeMap, BTreeSet};

use crate::journal::Record;
use crate::metrics::timeline::phase_of;

const QUEUE: usize = 0;
const REBASE: usize = 1;
const CI: usize = 2;
const MERGE: usize = 3;
const REWORK: usize = 4;

#[derive(Debug)]
struct Run {
  start: u64,
  bucket: usize,
  since: u64,
  step: Option<u64>,
  spent: [u64; 5],
  rework: bool,
}

impl Run {
  fn charge(&mut self, bucket: usize, from: u64, at: u64) {
    if let Some(spent) = self.spent.get_mut(bucket) {
      *spent = spent.saturating_add(at.saturating_sub(from));
    }
  }

  fn settle(&mut self, at: u64) {
    if self.step.is_none() {
      self.charge(self.bucket, self.since, at);
    }
    self.since = at;
  }
}

/// The measured durations, one value per merged issue.
#[derive(Debug, Default)]
pub(crate) struct Split {
  /// First `ready` to merged.
  pub total: Vec<u64>,
  /// Queue, rebase, CI, merge and rework, in that order.
  pub buckets: [Vec<u64>; 5],
  /// Total minus CI, for issues that never left the queue for rework.
  pub clean_excluding_ci: Vec<u64>,
}

/// Open runs by issue, and the issues already merged.
#[derive(Debug, Default)]
pub(crate) struct Ready {
  runs: BTreeMap<String, Run>,
  merged: BTreeSet<String>,
  split: Split,
}

impl Ready {
  /// Fold one in-scope record of `issue` at `at`.
  pub(crate) fn push(&mut self, issue: &str, record: &Record, at: u64) {
    if self.merged.contains(issue) {
      return;
    }
    let phase = (record.kind == "transition")
      .then(|| phase_of(&record.name, &record.note))
      .flatten();
    match (phase.as_deref(), record.name.as_str()) {
      (Some("ready"), _) => self.ready(issue, at),
      (_, "queue-left") => self.update(issue, at, |run| {
        run.bucket = REWORK;
        run.rework = true;
      }),
      (_, "merge-intent") => self.update(issue, at, |run| run.step = Some(at)),
      (_, "merge-done") => self.step_done(issue, &record.note, at),
      (_, "github-pr-merged") => self.finish(issue, at),
      _ => {}
    }
  }

  fn ready(&mut self, issue: &str, at: u64) {
    match self.runs.get_mut(issue) {
      Some(run) if run.bucket == REWORK => {
        run.settle(at);
        run.bucket = QUEUE;
      }
      Some(_) => {}
      None => {
        self.runs.insert(
          issue.to_owned(),
          Run {
            start: at,
            bucket: QUEUE,
            since: at,
            step: None,
            spent: [0; 5],
            rework: false,
          },
        );
      }
    }
  }

  fn update(&mut self, issue: &str, at: u64, change: impl FnOnce(&mut Run)) {
    if let Some(run) = self.runs.get_mut(issue) {
      run.settle(at);
      change(run);
    }
  }

  fn step_done(&mut self, issue: &str, note: &str, at: u64) {
    let outcome = note.split_once(": ").map_or(note, |(outcome, _)| outcome);
    if outcome.is_empty() {
      if let Some(run) = self.runs.get_mut(issue)
        && let Some(step) = run.step.take()
      {
        run.charge(MERGE, step, at);
        run.since = at;
      }
      self.finish(issue, at);
      return;
    }
    let Some(run) = self.runs.get_mut(issue) else {
      return;
    };
    if let Some(step) = run.step.take() {
      run.charge(if outcome == "rebased" { REBASE } else { QUEUE }, step, at);
    }
    run.since = at;
    run.bucket = match outcome {
      "rebased" | "waiting" | "stale" => CI,
      "handoff" | "fix" => {
        run.rework = true;
        REWORK
      }
      _ => QUEUE,
    };
  }

  fn finish(&mut self, issue: &str, at: u64) {
    self.merged.insert(issue.to_owned());
    let Some(mut run) = self.runs.remove(issue) else {
      return;
    };
    run.settle(at);
    if let Some(step) = run.step.take() {
      run.charge(MERGE, step, at);
    }
    let total = at.saturating_sub(run.start);
    self.split.total.push(total);
    for (values, spent) in self.split.buckets.iter_mut().zip(run.spent) {
      values.push(spent);
    }
    if !run.rework {
      let ci = run.spent.get(CI).copied().unwrap_or(0);
      self.split.clean_excluding_ci.push(total.saturating_sub(ci));
    }
  }

  /// The split of every issue that merged.
  pub(crate) fn finish_all(self) -> Split {
    self.split
  }
}

#[cfg(test)]
#[path = "tests/ready_test.rs"]
mod tests;
