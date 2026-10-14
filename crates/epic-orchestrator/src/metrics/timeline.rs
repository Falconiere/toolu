//! Lead time, phase time and relaunches per issue: a launch starts the clock,
//! a clean `cleanup-done` stops it, and every phase change (a worker report,
//! `github-babysit-ready`, `queue-left`) closes the previous phase.

use std::collections::BTreeMap;

use crate::journal::Record;

#[derive(Debug, Default)]
struct Issue {
  launched_at: Option<u64>,
  launches: u64,
  cleaned_at: Option<u64>,
  phase: Option<(String, u64)>,
}

/// Per-issue clocks folded from the journal.
#[derive(Debug, Default)]
pub(crate) struct Timeline {
  issues: BTreeMap<String, Issue>,
  phases: BTreeMap<String, u64>,
}

/// What the timeline measured.
#[derive(Debug, Default)]
pub(crate) struct Times {
  /// Launch to cleanup, per finished issue.
  pub lead: Vec<u64>,
  /// Milliseconds spent in each phase.
  pub phases: BTreeMap<String, u64>,
  /// Successful launches beyond the first, over all issues.
  pub relaunches: u64,
  /// Issues with a launch in scope.
  pub issues: u64,
}

impl Timeline {
  /// Fold one in-scope record of `issue` at `at` (Unix ms).
  pub(crate) fn push(&mut self, issue: &str, record: &Record, at: u64) {
    let entry = self.issues.entry(issue.to_owned()).or_default();
    match (record.kind.as_str(), record.name.as_str()) {
      ("action", "launch-intent") => {
        entry.launched_at.get_or_insert(at);
      }
      ("action", "launch-done") if record.note.is_empty() => {
        entry.launches = entry.launches.saturating_add(1);
      }
      ("action", "cleanup-done") if record.note.is_empty() && entry.cleaned_at.is_none() => {
        entry.cleaned_at = Some(at);
        close(&mut self.phases, entry, at);
      }
      ("transition", name) => {
        if let Some(phase) = phase_of(name, &record.note)
          && entry.cleaned_at.is_none()
        {
          close(&mut self.phases, entry, at);
          entry.phase = Some((phase, at));
        }
      }
      _ => {}
    }
  }

  /// The measures, with every open phase counted up to `now`.
  pub(crate) fn finish(mut self, now: u64) -> Times {
    let mut times = Times::default();
    for entry in self.issues.values_mut() {
      close(&mut self.phases, entry, now);
      if entry.launched_at.is_some() {
        times.issues = times.issues.saturating_add(1);
      }
      if let (Some(launched), Some(cleaned)) = (entry.launched_at, entry.cleaned_at) {
        times.lead.push(cleaned.saturating_sub(launched));
      }
      times.relaunches = times
        .relaunches
        .saturating_add(entry.launches.saturating_sub(1));
    }
    times.phases = self.phases;
    times
  }
}

/// The phase a transition moves its issue into, if it moves it.
pub(crate) fn phase_of(name: &str, note: &str) -> Option<String> {
  match name {
    "phase" if !note.is_empty() => Some(note.to_owned()),
    "github-babysit-ready" => Some("ready".to_owned()),
    "queue-left" => note
      .split_once(": ")
      .map_or(note, |(phase, _)| phase)
      .to_owned()
      .into(),
    _ => None,
  }
}

fn close(phases: &mut BTreeMap<String, u64>, entry: &mut Issue, at: u64) {
  if let Some((phase, since)) = entry.phase.take() {
    let spent = phases.entry(phase).or_default();
    *spent = spent.saturating_add(at.saturating_sub(since));
  }
}

#[cfg(test)]
#[path = "tests/timeline_test.rs"]
mod tests;
