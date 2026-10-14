//! Counted measures: prompts by reason (and the two kinds a budget forbids),
//! judgments with their waits, orchestrator wake-ups, job admission waits,
//! launch events and engine resource samples.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};

use crate::journal::Record;
use crate::metrics::stats::pct;

/// Judgment names that manage an item instead of raising one.
const CONTROL: [&str; 5] = ["reopened", "closed", "acknowledged", "answered", "reminder"];

#[derive(Debug, Default)]
struct IssuePrompts {
  handoff: bool,
  licenses: u64,
  last_next: String,
}

#[derive(Debug, Default)]
struct Job {
  open: Option<u64>,
  streak: Option<u64>,
}

/// Running counts over in-scope records.
#[derive(Debug, Default)]
pub(crate) struct Counts {
  prompts: BTreeMap<String, u64>,
  per_issue: BTreeMap<String, IssuePrompts>,
  while_waiting: u64,
  decided_checks: u64,
  rebase_without_conflict: u64,
  raised: BTreeMap<String, BTreeSet<String>>,
  closed: BTreeMap<String, Vec<u64>>,
  wakeups: (u64, u64),
  jobs: BTreeMap<String, Job>,
  job_tally: (u64, u64, u64),
  job_waits: Vec<u64>,
  launches: (u64, u64, u64),
  rounds: BTreeMap<String, [u64; 2]>,
  engine: Engine,
}

#[derive(Debug, Default)]
struct Engine {
  lifetimes: u64,
  max_rss: Option<u64>,
  cpu_done: u64,
  cpu_last: Option<u64>,
}

impl Counts {
  /// Fold one in-scope record of `issue` (the resolved issue key) at `at`.
  pub(crate) fn push(&mut self, issue: &str, record: &Record, at: u64) {
    match (record.kind.as_str(), record.name.as_str()) {
      ("prompt", name) => self.prompt(issue, name),
      ("judgment", name) => self.judgment(name, record),
      ("action", "merge-done") if record.note.starts_with("handoff") => {
        self.per_issue.entry(issue.to_owned()).or_default().handoff = true;
      }
      ("transition", "github-check") => self.check(issue, &record.note),
      ("action", "wait-reply") => {
        self.wakeups.0 = self.wakeups.0.saturating_add(1);
        self.wakeups.1 = self
          .wakeups
          .1
          .saturating_add(u64::from(record.note == "timeout"));
      }
      ("admission", "job") => self.attempt(&record.key, at),
      ("admission", "job-wait") => self.refused(&record.key, &record.note),
      ("action", "reroute") => self.launches.0 = self.launches.0.saturating_add(1),
      ("action", "host-limited") => self.launches.1 = self.launches.1.saturating_add(1),
      ("action", "launch-wait") if record.note.parse::<u64>().is_err() => {
        self.launches.2 = self.launches.2.saturating_add(1);
      }
      ("action", "engine-usage") => self.engine(&record.note),
      ("action", "rebase") => self.round(issue, &record.note),
      _ => {}
    }
  }

  fn prompt(&mut self, issue: &str, name: &str) {
    let count = self.prompts.entry(name.to_owned()).or_default();
    *count = count.saturating_add(1);
    let state = self.per_issue.entry(issue.to_owned()).or_default();
    match name {
      "rebase" if !std::mem::take(&mut state.handoff) => {
        self.rebase_without_conflict = self.rebase_without_conflict.saturating_add(1);
      }
      "babysit" if state.licenses > 0 => state.licenses = state.licenses.saturating_sub(1),
      "babysit" => self.while_waiting = self.while_waiting.saturating_add(1),
      "status" if matches!(state.last_next.as_str(), "keep-going" | "merge-queue") => {
        self.while_waiting = self.while_waiting.saturating_add(1);
      }
      _ => {}
    }
  }

  fn check(&mut self, issue: &str, note: &str) {
    let Some(next) = serde_json::from_str::<Value>(note)
      .ok()
      .and_then(|note| note.get("next").and_then(Value::as_str).map(str::to_owned))
    else {
      return;
    };
    self.decided_checks = self.decided_checks.saturating_add(1);
    let state = self.per_issue.entry(issue.to_owned()).or_default();
    if matches!(next.as_str(), "prompt" | "escalate") {
      state.licenses = state.licenses.saturating_add(1);
    }
    state.last_next = next;
  }

  fn judgment(&mut self, name: &str, record: &Record) {
    if !CONTROL.contains(&name) {
      self
        .raised
        .entry(name.to_owned())
        .or_default()
        .insert(record.token.clone());
      return;
    }
    if name != "closed" {
      return;
    }
    let field = |key: &str| {
      record
        .note
        .split(' ')
        .find_map(|part| part.strip_prefix(key))
        .map(str::to_owned)
    };
    if let Some(kind) = field("kind=") {
      let waited = field("waited_ms=").and_then(|ms| ms.parse::<u64>().ok());
      self.closed.entry(kind).or_default().extend(waited);
    }
  }

  fn attempt(&mut self, key: &str, at: u64) {
    self.job_tally.0 = self.job_tally.0.saturating_add(1);
    let job = self.jobs.entry(key.to_owned()).or_default();
    if let (Some(open), Some(streak)) = (job.open, job.streak) {
      self.job_waits.push(open.saturating_sub(streak));
      job.streak = None;
    }
    job.open = Some(at);
  }

  fn refused(&mut self, key: &str, note: &str) {
    self.job_tally.1 = self.job_tally.1.saturating_add(1);
    if note == "pressure" {
      self.job_tally.2 = self.job_tally.2.saturating_add(1);
    }
    let job = self.jobs.entry(key.to_owned()).or_default();
    if let Some(open) = job.open.take() {
      job.streak.get_or_insert(open);
    }
  }

  fn round(&mut self, issue: &str, note: &str) {
    let by = serde_json::from_str::<Value>(note)
      .ok()
      .and_then(|note| note.get("by").and_then(Value::as_str).map(str::to_owned));
    let slot = match by.as_deref() {
      Some("engine") => 0,
      Some("worker") => 1,
      _ => return,
    };
    let rounds = self.rounds.entry(issue.to_owned()).or_default();
    if let Some(count) = rounds.get_mut(slot) {
      *count = count.saturating_add(1);
    }
  }

  /// `{"engine", "worker", "issues"}`: rebase rounds by who performed them.
  pub(crate) fn rounds(&self) -> Value {
    let (engine, worker) = self.rounds.values().fold((0_u64, 0_u64), |(e, w), [a, b]| {
      (e.saturating_add(*a), w.saturating_add(*b))
    });
    let issues: serde_json::Map<String, Value> = self
      .rounds
      .iter()
      .map(|(key, [a, b])| (key.clone(), json!({"engine": a, "worker": b})))
      .collect();
    json!({"engine": engine, "worker": worker, "issues": issues})
  }

  fn engine(&mut self, note: &str) {
    let Ok(note) = serde_json::from_str::<Value>(note) else {
      return;
    };
    let engine = &mut self.engine;
    if note.get("event").and_then(Value::as_str) == Some("start") {
      engine.lifetimes = engine.lifetimes.saturating_add(1);
      engine.cpu_done = engine
        .cpu_done
        .saturating_add(engine.cpu_last.take().unwrap_or(0));
    }
    if let Some(rss) = note.get("max_rss_bytes").and_then(Value::as_u64) {
      engine.max_rss = Some(engine.max_rss.map_or(rss, |max| max.max(rss)));
    }
    if let Some(cpu) = note.get("cpu_ms").and_then(Value::as_u64) {
      engine.cpu_last = Some(cpu);
    }
  }

  /// The counted sections of the metrics document.
  pub(crate) fn finish(mut self, relaunches: u64) -> Value {
    for job in self.jobs.values() {
      if let (Some(open), Some(streak)) = (job.open, job.streak) {
        self.job_waits.push(open.saturating_sub(streak));
      }
    }
    let mut judgments = serde_json::Map::new();
    let kinds: BTreeSet<&String> = self.raised.keys().chain(self.closed.keys()).collect();
    for kind in kinds {
      let waits = self.closed.get(kind).map_or(&[][..], Vec::as_slice);
      let raised = self.raised.get(kind).map_or(0, BTreeSet::len);
      judgments.insert(
        kind.clone(),
        json!({"raised": raised, "closed": waits.len(), "wait_ms": pct(waits)}),
      );
    }
    let engine = &self.engine;
    let cpu = engine
      .cpu_last
      .map(|last| engine.cpu_done.saturating_add(last));
    json!({
      "prompts": {
        "by_reason": self.prompts,
        "decided_checks": self.decided_checks,
        "while_waiting": self.while_waiting,
        "rebase_without_conflict": self.rebase_without_conflict,
      },
      "judgments": judgments,
      "wakeups": {"total": self.wakeups.0, "judgment_free": self.wakeups.1},
      "jobs": {
        "attempts": self.job_tally.0, "waits": self.job_tally.1,
        "pressure_holds": self.job_tally.2, "wait_ms": pct(&self.job_waits),
      },
      "launches": {
        "relaunches": relaunches, "host_switches": self.launches.0,
        "cooldowns": self.launches.1, "waits": self.launches.2,
      },
      "engine": {"lifetimes": engine.lifetimes, "max_rss_bytes": engine.max_rss, "cpu_ms": cpu},
    })
  }
}

#[cfg(test)]
#[path = "tests/counts_test.rs"]
mod tests;
