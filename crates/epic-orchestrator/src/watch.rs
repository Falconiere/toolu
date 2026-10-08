//! Fixed GitHub watch deadlines, independent of the engine maintenance tick.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::journal::Record;
use crate::model::Step;
use crate::model::World;

/// One interval for every GitHub watch, with no adaptive backoff (#447).
pub(crate) const CHECK_MS: u64 = 180_000;

/// A registered GitHub resource the engine may check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub(crate) enum Kind {
  /// A pull request whose worker reported a PR number.
  Pr {
    key: String,
    repo: String,
    number: u64,
  },
  /// The base branch of a repository with a queued PR.
  Base { repo: String, branch: String },
  /// A registered epic's sub-issues.
  Epic {
    key: String,
    repo: String,
    number: u64,
  },
  /// An open blocker of a registered epic.
  Blocker {
    key: String,
    repo: String,
    number: u64,
  },
}

/// Why one request is due.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Cause {
  /// A worker report or merge added an extra check.
  Immediate,
  /// The fixed timer elapsed.
  Scheduled,
}

/// The saved state of one GitHub resource.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Watch {
  /// What the watch observes.
  pub kind: Kind,
  /// Next fixed scheduled slot in Unix milliseconds.
  pub next_at_ms: u64,
  /// Last completed attempt in Unix milliseconds.
  pub last_at_ms: Option<u64>,
  /// No network call until this time after a rate limit.
  pub retry_after_until_ms: u64,
  /// An event requested a check outside the fixed schedule.
  pub immediate: bool,
  /// Conditional REST `ETag` values by request path.
  pub etags: BTreeMap<String, String>,
}

impl Watch {
  /// Start watching at `now_ms` and request an initial extra check.
  pub(crate) fn new(kind: Kind, now_ms: u64) -> Self {
    Self {
      kind,
      next_at_ms: now_ms.saturating_add(CHECK_MS),
      last_at_ms: None,
      retry_after_until_ms: 0,
      immediate: true,
      etags: BTreeMap::new(),
    }
  }

  /// Keep the scheduled deadline while requesting another check.
  pub(crate) fn request_immediate(&mut self) {
    self.immediate = true;
  }

  /// Consume a due check and advance from the previous scheduled slot.
  pub(crate) fn take_due(&mut self, now_ms: u64) -> Option<Cause> {
    if now_ms < self.retry_after_until_ms {
      return None;
    }
    if self.immediate {
      self.immediate = false;
      self.last_at_ms = Some(now_ms);
      return Some(Cause::Immediate);
    }
    if now_ms < self.next_at_ms {
      return None;
    }
    let slots = now_ms.saturating_sub(self.next_at_ms) / CHECK_MS + 1;
    self.next_at_ms = self
      .next_at_ms
      .saturating_add(slots.saturating_mul(CHECK_MS));
    self.last_at_ms = Some(now_ms);
    Some(Cause::Scheduled)
  }

  /// Next time a state-loop wake may send a request.
  pub(crate) fn due_at_ms(&self, now_ms: u64) -> u64 {
    let due = if self.immediate {
      now_ms
    } else {
      self.next_at_ms
    };
    due.max(self.retry_after_until_ms)
  }
}

/// Record the due inputs; the detector consumes them in the next workstream.
pub(crate) fn take_due(world: &mut World) {
  for (key, watch) in &mut world.watches {
    if let Some(cause) = watch.take_due(world.now_ms) {
      let name = match cause {
        Cause::Immediate => "github-immediate",
        Cause::Scheduled => "github-scheduled",
      };
      world
        .outbox
        .push_back(Step::Journal(Record::new("transition", name, key, "", "")));
    }
  }
}

/// Reconcile saved watches with the currently registered, waiting PRs.
pub(crate) fn sync(world: &mut World) {
  let mut wanted = BTreeMap::new();
  for issue in world.issues.values() {
    if issue.pr.is_none() || issue.repo.is_empty() || !waiting(&issue.phase, &issue.stage) {
      continue;
    }
    if let Some(pr) = issue.pr {
      let kind = Kind::Pr {
        key: issue.key.clone(),
        repo: issue.repo.clone(),
        number: pr,
      };
      wanted.insert(id(&kind), kind);
    }
    if issue.phase == "ready" && !issue.base.is_empty() {
      let kind = Kind::Base {
        repo: issue.repo.clone(),
        branch: issue.base.clone(),
      };
      wanted.insert(id(&kind), kind);
    }
    if let Some(reference) = world.epic_refs.get(&issue.epic)
      && let Some((repo, number)) = reference_parts(reference)
    {
      let kind = Kind::Epic {
        key: issue.epic.clone(),
        repo: repo.to_owned(),
        number,
      };
      wanted.insert(id(&kind), kind);
    }
    for reference in &issue.blockers {
      if let Some((repo, number)) = reference_parts(reference) {
        let kind = Kind::Blocker {
          key: issue.epic.clone(),
          repo: repo.to_owned(),
          number,
        };
        wanted.insert(id(&kind), kind);
      }
    }
  }
  world.watches.retain(|key, _| wanted.contains_key(key));
  for (key, kind) in wanted {
    world
      .watches
      .entry(key)
      .or_insert_with(|| Watch::new(kind, world.now_ms));
  }
}

fn waiting(phase: &str, stage: &str) -> bool {
  matches!(phase, "pr-open" | "babysit" | "ready") && !matches!(stage, "merged" | "closed")
}

fn reference_parts(reference: &str) -> Option<(&str, u64)> {
  let (repo, number) = reference.rsplit_once('#')?;
  let (owner, name) = repo.split_once('/')?;
  if owner.is_empty() || name.is_empty() || name.contains('/') {
    return None;
  }
  Some((repo, number.parse().ok()?))
}

fn id(kind: &Kind) -> String {
  match kind {
    Kind::Pr { repo, number, .. } => format!("pr:{repo}#{number}"),
    Kind::Base { repo, branch } => format!("base:{repo}@{branch}"),
    Kind::Epic { repo, number, .. } => format!("epic:{repo}#{number}"),
    Kind::Blocker { repo, number, .. } => format!("blocker:{repo}#{number}"),
  }
}

#[cfg(test)]
#[path = "tests/watch_test.rs"]
mod tests;
