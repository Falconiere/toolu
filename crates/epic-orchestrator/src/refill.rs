//! Refill: when an epic has free slots, form a batch of its ready issues in
//! `pick_batch` order (chain length, unblocked count, repo load, repo,
//! number), cut to the free slots, and arm one launch at a time from it.
//! Waiting members leave the batch and come back as fresh candidates.

use crate::epics::{Member, max_parallel};
use crate::graph::{occupies_slot, pick_batch};
use crate::journal::Record;
use crate::logic::{paused, token};
use crate::model::{Action, Issue, Step, World, fresh};

/// A waiting launch is tried again after one tick.
const RETRY_MS: u64 = 30_000;

/// Whether `issue` holds a slot: an occupied stage, or a pending action.
pub(crate) fn occupies(issue: &Issue) -> bool {
  issue.pending.is_some() || occupies_slot(Some(&issue.stage))
}

/// Whether `issue` is done (`merged` or `abandoned`).
pub(crate) fn done(issue: &Issue) -> bool {
  matches!(issue.stage.as_str(), "merged" | "abandoned")
}

/// Whether `issue` may join a batch: never launched, open in the tracker,
/// unblocked, not held, deferred, paused or waiting out a retry.
pub(crate) fn ready(world: &World, issue: &Issue) -> bool {
  let unblocked = issue
    .blockers
    .iter()
    .all(|blocker| world.issues.get(blocker).is_some_and(done));
  issue.stage.is_empty()
    && issue.graph_status != "done"
    && issue.pending.is_none()
    && issue.hold.is_none()
    && issue.deferred.is_none()
    && unblocked
    && !paused(world, issue)
    && world.now_ms >= issue.retry_at_ms
}

fn free_slots(world: &World, epic: &str) -> usize {
  let taken = world
    .issues
    .values()
    .filter(|issue| issue.epic == epic && occupies(issue))
    .count();
  usize::try_from(max_parallel(world, epic))
    .unwrap_or(0)
    .saturating_sub(taken)
}

/// The ready issues of `epic` in pick order, cut to `slots`.
fn candidates(world: &World, epic: &str, slots: usize) -> Vec<String> {
  let ready: Vec<&Issue> = world
    .issues
    .values()
    .filter(|issue| issue.epic == epic && ready(world, issue))
    .collect();
  let Some(graph) = world.epics.get(epic).and_then(|epic| epic.graph.as_ref()) else {
    return ready
      .iter()
      .take(slots)
      .map(|issue| issue.key.clone())
      .collect();
  };
  let reference = |key: &str| {
    graph
      .issues
      .iter()
      .find(|item| item.key == key)
      .map(|item| item.reference.clone())
  };
  let refs: Vec<String> = ready
    .iter()
    .filter_map(|issue| reference(&issue.key))
    .collect();
  let flying: Vec<String> = world
    .issues
    .values()
    .filter(|issue| issue.epic == epic && occupies(issue))
    .filter_map(|issue| reference(&issue.key))
    .collect();
  pick_batch(&refs, &graph.issues, &flying, slots)
    .iter()
    .filter_map(|picked| {
      graph
        .issues
        .iter()
        .find(|item| item.reference == *picked)
        .map(|item| item.key.clone())
    })
    .collect()
}

/// Members that can no longer launch are `skipped`; the next unfinished one.
fn next_member(world: &mut World, epic: &str) -> Option<String> {
  let members: Vec<String> = world
    .epics
    .get(epic)?
    .batch
    .iter()
    .filter(|member| member.outcome.is_none())
    .map(|member| member.key.clone())
    .collect();
  for key in members {
    let launchable = world
      .issues
      .get(&key)
      .is_some_and(|issue| ready(world, issue));
    if launchable {
      return Some(key);
    }
    set_outcome(world, &key, "skipped");
  }
  None
}

/// Record how `key`'s launch ended in its epic's batch.
pub(crate) fn set_outcome(world: &mut World, key: &str, outcome: &str) {
  let Some(epic) = world.issues.get(key).map(|issue| issue.epic.clone()) else {
    return;
  };
  if let Some(member) = world
    .epics
    .get_mut(&epic)
    .and_then(|epic| epic.batch.iter_mut().find(|member| member.key == key))
  {
    member.outcome = Some(outcome.to_owned());
  }
}

/// Count one more wait for `key`; the first of a streak is journaled.
pub(crate) fn note_wait(world: &mut World, key: &str, reason: &str) {
  let first = world
    .issues
    .get(key)
    .is_some_and(|issue| issue.wait_streak == 0);
  if let Some(issue) = world.issues.get_mut(key) {
    issue.wait_streak = issue.wait_streak.saturating_add(1);
    issue.retry_at_ms = world.now_ms.saturating_add(RETRY_MS);
  }
  set_outcome(world, key, "wait");
  if first {
    world.outbox.push_back(Step::Journal(Record::new(
      "action",
      "launch-wait",
      key,
      "",
      reason,
    )));
  }
}

/// Mark a relaunch of a running issue (after its agent was lost, or a
/// reroute) that had to wait, so `rearm` retries it once `retry_at_ms`
/// passes and the record keeps it across a restart. A fresh launch leaves
/// its batch and comes back through `ready` instead.
pub(crate) fn mark_relaunch(world: &mut World, key: &str, reroute: bool) {
  let Some(issue) = world
    .issues
    .get_mut(key)
    .filter(|issue| !issue.stage.is_empty())
  else {
    return;
  };
  issue.relaunch = Some(if reroute { "reroute" } else { "lost" }.to_owned());
  world.outbox.push_back(Step::Issue {
    key: key.to_owned(),
  });
}

/// Arm the first waiting relaunch of `epic` whose retry time has passed;
/// whether one was armed.
fn rearm(world: &mut World, epic: &str) -> bool {
  let due = world.issues.values().find(|issue| {
    issue.epic == epic
      && issue.stage == "running"
      && issue.relaunch.is_some()
      && issue.pending.is_none()
      && issue.hold.is_none()
      && issue.deferred.is_none()
      && world.now_ms >= issue.retry_at_ms
  });
  let Some((key, reroute)) = due.map(|issue| {
    (
      issue.key.clone(),
      issue.relaunch.as_deref() == Some("reroute"),
    )
  }) else {
    return false;
  };
  if crate::github_budget::effects_held(world) {
    note_wait(world, &key, "GitHub budget under its floor");
    return true;
  }
  let token = token(world);
  let mut pending = fresh(Action::Launch, token);
  pending.reroute = reroute;
  if let Some(issue) = world.issues.get_mut(&key) {
    issue.pending = Some(pending);
  }
  true
}

/// A streak of waits ended; journal its length once.
pub(crate) fn end_wait(world: &mut World, key: &str) {
  let streak = world.issues.get(key).map_or(0, |issue| issue.wait_streak);
  if streak == 0 {
    return;
  }
  if let Some(issue) = world.issues.get_mut(key) {
    issue.wait_streak = 0;
  }
  world.outbox.push_back(Step::Journal(Record::new(
    "action",
    "launch-wait",
    key,
    "",
    &streak.to_string(),
  )));
}

fn launching(world: &World, epic: &str) -> bool {
  world.issues.values().any(|issue| {
    issue.epic == epic
      && issue
        .pending
        .as_ref()
        .is_some_and(|pending| pending.action == Action::Launch)
  })
}

fn arm(world: &mut World, key: &str) {
  if crate::github_budget::effects_held(world) {
    note_wait(world, key, "GitHub budget under its floor");
    return;
  }
  let token = token(world);
  if let Some(issue) = world.issues.get_mut(key) {
    issue.pending = Some(fresh(Action::Launch, token));
  }
}

/// Form or continue each epic's batch and arm its next launch.
pub(crate) fn refill(world: &mut World) {
  if world.draining || world.paused_all {
    return;
  }
  let epics: Vec<String> = world
    .issues
    .values()
    .map(|issue| issue.epic.clone())
    .collect::<std::collections::BTreeSet<_>>()
    .into_iter()
    .collect();
  for epic in epics {
    if world.paused.contains(&epic) || launching(world, &epic) || rearm(world, &epic) {
      continue;
    }
    if let Some(key) = next_member(world, &epic) {
      arm(world, &key);
      continue;
    }
    form_batch(world, &epic);
  }
}

/// A new batch from `epic`'s ready issues, cut to its free slots, and its
/// first launch armed.
fn form_batch(world: &mut World, epic: &str) {
  let slots = free_slots(world, epic);
  let picked = candidates(world, epic, slots);
  let Some(first) = picked.first().cloned() else {
    return;
  };
  let batch: Vec<Member> = picked
    .iter()
    .map(|key| Member {
      key: key.clone(),
      outcome: None,
    })
    .collect();
  if let Some(state) = world.epics.get_mut(epic) {
    state.batch = batch;
  } else {
    // An epic without a graph document still launches in order.
    let state_dir = world
      .issues
      .values()
      .find(|issue| issue.epic == epic)
      .map(|issue| issue.state_dir.clone())
      .unwrap_or_default();
    let mut state = crate::epics::Epic::new(epic, &state_dir, world.now_ms, None);
    state.max_parallel = world.max_parallel;
    state.batch = batch;
    world.epics.insert(epic.to_owned(), state);
  }
  arm(world, &first);
}

#[cfg(test)]
#[path = "tests/refill_test.rs"]
mod tests;
