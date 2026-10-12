//! The merge queue (#448): one per repository, derived from the engine's
//! issues. A ready issue with a pull request is queued; each repository's
//! queue is ordered by how much downstream work its merge unblocks (the
//! graph's `unblocks`), then by when it became ready, then by key. Only the
//! head of a queue is gated, rebased and merged.

use std::collections::BTreeMap;
use std::time::{Duration, UNIX_EPOCH};

use serde_json::{Value, json};
use toolu_state::time::iso_millis;

use crate::model::{Issue, World};

/// One queued pull request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Entry {
  /// Issue key.
  pub key: String,
  /// Epic key.
  pub epic: String,
  /// Pull request number.
  pub pr: u64,
  /// Issues that transitively wait on this one.
  pub unblocks: u64,
  /// When it joined the queue, milliseconds; `None` sorts last.
  pub queued_at_ms: Option<u64>,
}

/// Whether `issue` waits in its repository's merge queue.
pub(crate) fn queued(issue: &Issue) -> bool {
  issue.phase == "ready"
    && matches!(issue.stage.as_str(), "running" | "awaiting_merge")
    && issue.pr.is_some()
    && !issue.repo.is_empty()
}

fn unblocks(world: &World, issue: &Issue) -> u64 {
  world
    .epics
    .get(&issue.epic)
    .and_then(|epic| epic.graph.as_ref())
    .and_then(|graph| graph.find(&issue.key))
    .map_or(0, |node| node.unblocks)
}

/// Every repository's queue, in merge order.
pub(crate) fn queues(world: &World) -> BTreeMap<String, Vec<Entry>> {
  let mut by_repo = BTreeMap::<String, Vec<Entry>>::new();
  for issue in world.issues.values().filter(|issue| queued(issue)) {
    let entry = Entry {
      key: issue.key.clone(),
      epic: issue.epic.clone(),
      pr: issue.pr.unwrap_or(0),
      unblocks: unblocks(world, issue),
      queued_at_ms: issue.queued_at_ms,
    };
    by_repo.entry(issue.repo.clone()).or_default().push(entry);
  }
  for entries in by_repo.values_mut() {
    entries.sort_by(|a, b| {
      b.unblocks
        .cmp(&a.unblocks)
        .then(
          a.queued_at_ms
            .unwrap_or(u64::MAX)
            .cmp(&b.queued_at_ms.unwrap_or(u64::MAX)),
        )
        .then(a.key.cmp(&b.key))
    });
  }
  by_repo
}

fn paused(world: &World, epic: &str) -> bool {
  world.paused_all || world.paused.contains(epic)
}

/// The issue the engine works next in `repo`: the first entry whose epic is
/// not paused.
pub(crate) fn head(world: &World, repo: &str) -> Option<String> {
  queues(world)
    .remove(repo)?
    .into_iter()
    .find(|entry| !paused(world, &entry.epic))
    .map(|entry| entry.key)
}

/// Whether `key` is the head of its repository's queue.
pub(crate) fn is_head(world: &World, key: &str) -> bool {
  world
    .issues
    .get(key)
    .filter(|issue| queued(issue))
    .and_then(|issue| head(world, &issue.repo))
    .is_some_and(|head| head == key)
}

/// The queue document of `/api/v1/queue` and `toolu epic queue`, optionally
/// limited to one epic's entries.
pub(crate) fn document(world: &World, epic: Option<&str>) -> Value {
  let repositories: Vec<Value> = queues(world)
    .into_iter()
    .filter_map(|(repo, entries)| {
      let head = head(world, &repo);
      let items: Vec<Value> = entries
        .iter()
        .enumerate()
        .filter(|(_, entry)| epic.is_none_or(|wanted| entry.epic == wanted))
        .map(|(index, entry)| item(world, index, entry, head.as_deref()))
        .collect();
      (!items.is_empty()).then(|| json!({"repo": repo, "items": items}))
    })
    .collect();
  json!({"schema": 1, "repositories": repositories})
}

fn item(world: &World, index: usize, entry: &Entry, head: Option<&str>) -> Value {
  let at = entry
    .queued_at_ms
    .map(|ms| iso_millis(UNIX_EPOCH + Duration::from_millis(ms)));
  let rounds = world
    .issues
    .get(&entry.key)
    .map(|issue| issue.rebase_rounds)
    .unwrap_or_default();
  json!({
    "position": index.saturating_add(1),
    "key": entry.key,
    "epic": entry.epic,
    "pr": entry.pr,
    "unblocks": entry.unblocks,
    "queued_at": at,
    "head": head == Some(entry.key.as_str()),
    "rebase_rounds": {"engine": rounds.engine, "worker": rounds.worker},
  })
}

#[cfg(test)]
#[path = "tests/queue_test.rs"]
mod tests;
