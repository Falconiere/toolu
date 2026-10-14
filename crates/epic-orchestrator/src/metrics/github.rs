//! GitHub measures from `github-check` and `github-error` notes: request
//! counts and points, the share answered `304`, the interval each attempt
//! scheduled next (`nextAt`), the observed gap between scheduled attempts,
//! and how fast the engine reacted to a green verdict.

use std::collections::BTreeMap;

use serde_json::{Value, json};

use crate::journal::Record;
use crate::metrics::scope::Scope;
use crate::metrics::stats::{pct, share};

#[derive(Debug, Clone, Copy)]
struct Attempt {
  at: u64,
  held: bool,
  lifetime: u64,
}

#[derive(Debug, Clone, Copy)]
struct Verdict {
  at: u64,
  checked_again: bool,
  queue_busy: bool,
}

/// Running GitHub figures.
#[derive(Debug, Default)]
pub(crate) struct GitHub {
  checks: u64,
  errors: u64,
  sums: [u64; 4],
  next_gaps: Vec<u64>,
  gaps: Vec<u64>,
  last: BTreeMap<String, Attempt>,
  lifetime: u64,
  verdicts: BTreeMap<String, Verdict>,
  reactions: Vec<u64>,
  late: u64,
}

const NOTE_SUMS: [&str; 4] = [
  "restFresh",
  "restNotModified",
  "restPoints",
  "graphqlPoints",
];

impl GitHub {
  /// Fold one in-scope record; `issue` is its resolved issue key.
  pub(crate) fn push(&mut self, scope: &Scope, issue: &str, record: &Record, at: u64) {
    match record.name.as_str() {
      "github-check" | "github-error" => self.attempt(issue, record, at),
      "github-babysit-ready" => {
        let verdict = Verdict {
          at,
          checked_again: false,
          queue_busy: false,
        };
        self.verdicts.insert(issue.to_owned(), verdict);
      }
      "merge-intent" => self.intent(scope, issue, at),
      "engine-usage" if record.note.contains("\"start\"") => {
        self.lifetime = self.lifetime.saturating_add(1);
      }
      _ => {}
    }
  }

  fn attempt(&mut self, issue: &str, record: &Record, at: u64) {
    let note: Value = serde_json::from_str(&record.note).unwrap_or(Value::Null);
    if record.name == "github-check" {
      self.checks = self.checks.saturating_add(1);
      if let Some(verdict) = self.verdicts.get_mut(issue) {
        verdict.checked_again = true;
      }
    } else {
      self.errors = self.errors.saturating_add(1);
    }
    for (sum, field) in self.sums.iter_mut().zip(NOTE_SUMS) {
      *sum = sum.saturating_add(note.get(field).and_then(Value::as_u64).unwrap_or(0));
    }
    if let Some(next) = note.get("nextAt").and_then(Value::as_u64) {
      self.next_gaps.push(next.saturating_sub(at));
    }
    if note.get("cause").and_then(Value::as_str) != Some("scheduled") {
      return;
    }
    let held = ["retryAfter", "graphqlRetryAfter"]
      .iter()
      .any(|field| note.get(field).is_some_and(|value| !value.is_null()));
    let current = Attempt {
      at,
      held,
      lifetime: self.lifetime,
    };
    if let Some(previous) = self.last.insert(record.key.clone(), current)
      && previous.lifetime == current.lifetime
      && !previous.held
    {
      self.gaps.push(at.saturating_sub(previous.at));
    }
  }

  fn intent(&mut self, scope: &Scope, issue: &str, at: u64) {
    if let Some(verdict) = self.verdicts.remove(issue) {
      self.reactions.push(at.saturating_sub(verdict.at));
      if verdict.checked_again && !verdict.queue_busy {
        self.late = self.late.saturating_add(1);
      }
    }
    let Some(repo) = scope.repo(issue) else {
      return;
    };
    for (other, verdict) in &mut self.verdicts {
      if scope.repo(other) == Some(repo) {
        verdict.queue_busy = true;
      }
    }
  }

  /// The `github` section of the metrics document.
  pub(crate) fn finish(&self) -> Value {
    let [fresh, not_modified, rest_points, graphql_points] = self.sums;
    json!({
      "checks": self.checks,
      "errors": self.errors,
      "rest_fresh": fresh,
      "rest_not_modified": not_modified,
      "not_modified_share": share(not_modified, fresh.saturating_add(not_modified)),
      "rest_points": rest_points,
      "graphql_points": graphql_points,
      "next_gap_ms": pct(&self.next_gaps),
      "check_gap_ms": pct(&self.gaps),
      "reaction_ms": pct(&self.reactions),
      "late_reactions": self.late,
    })
  }
}

#[cfg(test)]
#[path = "tests/github_test.rs"]
mod tests;
