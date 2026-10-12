//! Per-issue transitions. The server performs the steps this module returns.

use crate::journal::Record;
use crate::model::{Action, Issue, Pending, Report, Step, World, fresh};
use crate::note::bounded_note;

pub(crate) fn token(world: &mut World) -> String {
  world.seq = world.seq.saturating_add(1);
  format!("t{}", world.seq)
}

pub(crate) fn paused(world: &World, issue: &Issue) -> bool {
  world.paused_all || world.paused.contains(&issue.epic)
}

fn judgment_phase(phase: &str) -> bool {
  matches!(phase, "needs-human" | "failed" | "blocked")
}

/// Record a worker report and return the snapshots and journal lines to write.
pub(crate) fn apply_report(world: &mut World, report: &Report) -> Vec<Step> {
  let changed = phase_changed(world, report);
  crate::attention::close_key(world, &report.key);
  let now_ms = world.now_ms;
  let issue = issue_mut(world, report);
  // A worker that reports is running, whether or not the engine launched it:
  // never launch a second agent for it.
  let adopted = issue.stage.is_empty();
  if adopted {
    "running".clone_into(&mut issue.stage);
  }
  report.phase.clone_into(&mut issue.phase);
  issue.note = bounded_note(&report.note);
  if let Some(pr) = report.pr {
    issue.pr = Some(pr);
  }
  if changed {
    issue.stall_nudged = false;
    issue.phase_at_ms = now_ms;
    issue.nudge_at_ms = 0;
    issue.deferred = None;
  }
  let mut steps = vec![
    Step::Status {
      key: report.key.clone(),
    },
    Step::Journal(Record::new(
      "transition",
      "phase",
      &report.key,
      "",
      &report.phase,
    )),
  ];
  if adopted {
    steps.push(Step::Issue {
      key: report.key.clone(),
    });
  }
  // A rate-limited failure cools the host and reroutes instead of a judgment.
  let limited = report.phase == "failed" && crate::limit::rate_limited(&report.note);
  if judgment_phase(&report.phase) && !limited {
    push_attention(world, &report.phase, &report.key, &report.note);
  } else if report.phase == "ready" {
    arm_merge(world, &report.key);
  }
  steps.extend(world.outbox.drain(..));
  steps
}

fn phase_changed(world: &World, report: &Report) -> bool {
  world
    .issues
    .get(&report.key)
    .is_none_or(|issue| issue.phase != report.phase)
}

fn issue_mut<'a>(world: &'a mut World, report: &Report) -> &'a mut Issue {
  let now = world.now_ms;
  world
    .issues
    .entry(report.key.clone())
    .or_insert_with(|| Issue::blank(&report.key, &report.epic, &report.state_dir, now))
}

pub(crate) fn arm_merge(world: &mut World, key: &str) {
  let Some(issue) = world.issues.get(key) else {
    return;
  };
  if issue.stage != "running" || issue.pending.is_some() || issue.deferred.is_some() {
    return;
  }
  let token = token(world);
  if let Some(issue) = world.issues.get_mut(key) {
    issue.pending = Some(fresh(Action::Merge, token));
  }
}

/// Arm merge for every ready issue that is allowed to run.
pub(crate) fn arm_ready(world: &mut World) {
  if world.paused_all {
    return;
  }
  let keys: Vec<String> = world
    .issues
    .iter()
    .filter(|(_, issue)| issue.phase == "ready" && !world.paused.contains(&issue.epic))
    .map(|(key, _)| key.clone())
    .collect();
  for key in keys {
    arm_merge(world, &key);
  }
}

pub(crate) fn push_attention(world: &mut World, kind: &str, key: &str, note: &str) {
  let _token = crate::attention::raise(world, kind, key, note);
}

/// The next thing to do, if the machine is not idle.
pub(crate) fn next(world: &mut World) -> Option<Step> {
  raise_cycle(world);
  if let Some(step) = world.outbox.pop_front() {
    return Some(step);
  }
  if let Some(step) = pending_step(world) {
    return Some(step);
  }
  crate::refill::refill(world);
  if let Some(step) = world.outbox.pop_front() {
    return Some(step);
  }
  pending_step(world)
}

fn raise_cycle(world: &mut World) {
  if world.cycle != crate::model::Cycle::Open {
    return;
  }
  world.cycle = crate::model::Cycle::Sent;
  push_attention(world, "dependency-cycle", "", "dependency cycle");
}

fn pending_step(world: &World) -> Option<Step> {
  issue_step(world).or_else(|| crate::refresh::epic_step(world))
}

fn issue_step(world: &World) -> Option<Step> {
  world.issues.values().find_map(|issue| {
    if paused(world, issue) {
      return None;
    }
    issue.pending.as_ref().and_then(|pending| {
      // A call does not start while one runs or the engine drains.
      let blocked_call = pending.phase == 1 && (world.effect_busy || world.draining);
      let held = crate::github_budget::effects_held(world)
        && matches!(pending.action, Action::Launch | Action::Merge);
      (!blocked_call && !held).then(|| step_for(issue, pending))
    })
  })
}

fn step_for(issue: &Issue, pending: &Pending) -> Step {
  let name = pending.action.name();
  match pending.phase {
    0 => journal_step(issue, &format!("{name}-intent"), &pending.token),
    2 => journal_step(issue, &format!("{name}-done"), &pending.token),
    _ if pending.recovered => call_step(issue, Action::Reconcile, &pending.token),
    _ => call_step(issue, pending.action, &pending.token),
  }
}

fn journal_step(issue: &Issue, name: &str, token: &str) -> Step {
  Step::Journal(Record::new("action", name, &issue.key, token, ""))
}

fn call_step(issue: &Issue, action: Action, token: &str) -> Step {
  Step::Call {
    key: issue.key.clone(),
    action,
    token: token.to_owned(),
  }
}

/// The intent or done line was written.
pub(crate) fn commit_journal(world: &mut World, record: &Record) -> Vec<Step> {
  if crate::refresh::epic_record(&record.name) {
    return crate::refresh::commit_journal(world, record);
  }
  if record.name.ends_with("-intent") {
    advance_phase(world, &record.key, 1);
    return Vec::new();
  }
  // A launch that ended without a stage change (held, waiting, failed or
  // uncertain) already cleared or replaced its pending action.
  if record.name.ends_with("-done") && !record.note.is_empty() {
    return Vec::new();
  }
  if record.name.ends_with("-done") {
    let steps = finish_action(world, &record.key);
    // A finished cleanup refreshes its epic, which may then close it.
    if record.name == "cleanup-done"
      && let Some(epic) = world
        .issues
        .get(&record.key)
        .map(|issue| issue.epic.clone())
    {
      crate::refresh::arm(world, &epic);
    }
    return steps;
  }
  Vec::new()
}

pub(crate) fn advance_phase(world: &mut World, key: &str, phase: u8) {
  if let Some(pending) = world
    .issues
    .get_mut(key)
    .and_then(|issue| issue.pending.as_mut())
  {
    pending.phase = phase;
  }
}

pub(crate) fn finish_action(world: &mut World, key: &str) -> Vec<Step> {
  let follow = take_follow(world, key);
  if let Some(action) = follow {
    let token = token(world);
    if let Some(issue) = world.issues.get_mut(key) {
      issue.pending = Some(fresh(action, token));
    }
  }
  vec![Step::Issue {
    key: key.to_owned(),
  }]
}

fn take_follow(world: &mut World, key: &str) -> Option<Action> {
  let issue = world.issues.get_mut(key)?;
  let pending = issue.pending.take()?;
  chain(&mut issue.stage, &pending)
}

fn chain(stage: &mut String, pending: &Pending) -> Option<Action> {
  match pending.action {
    Action::Merge => {
      "cleaning".clone_into(stage);
      Some(Action::Cleanup)
    }
    Action::Cleanup => {
      "merged".clone_into(stage);
      None
    }
    Action::Launch => {
      "running".clone_into(stage);
      None
    }
    Action::Checkpoint => pending.follow,
    Action::Reconcile | Action::Prompt | Action::Refresh | Action::Close => None,
  }
}

/// Insert a graph issue if the engine has not seen it.
pub(crate) fn ensure_issue<'a>(
  world: &'a mut World,
  key: &str,
  epic: &str,
  state_dir: &str,
) -> &'a mut Issue {
  let now = world.now_ms;
  world
    .issues
    .entry(key.to_owned())
    .or_insert_with(|| Issue::blank(key, epic, state_dir, now))
}

#[cfg(test)]
#[path = "tests/logic_test.rs"]
mod tests;
