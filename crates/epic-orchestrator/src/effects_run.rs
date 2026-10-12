//! The body of each engine effect, built on the state thread from what the
//! effect needs and run by the executor: the checkpoint and the `STATUS?`
//! prompt, the launch, the cleanup and the epic refresh and close in process,
//! and the script seam (`TOOLU_EPIC_SCRIPTS`) for merge and its reconcile.

use std::path::PathBuf;

use serde_json::json;

use crate::checkpoint::snapshot_active;
use crate::effects::invoke;
use crate::executor::{Call, Job};
use crate::model::Action;
use crate::server::Engine;

/// The effect body of `call` in `engine`.
pub(crate) fn job(engine: &Engine, call: &Call) -> Job {
  let issue = engine.world.issues.get(&call.key);
  if issue.is_none()
    && let Some(epic) = engine.world.epics.get(&call.key)
  {
    return crate::effects_epic::job(call.action, epic_job(engine, epic));
  }
  match call.action {
    Action::Checkpoint => {
      let state = issue.map(|issue| PathBuf::from(&issue.state_dir));
      checkpoint_job(state, call.key.clone(), engine.git_trace.clone())
    }
    Action::Launch => crate::effects_launch::job(crate::effects_launch::LaunchJob {
      state_dir: issue
        .map(|issue| PathBuf::from(&issue.state_dir))
        .unwrap_or_default(),
      key: call.key.clone(),
      ahead: crate::holds::coupling_ahead(&engine.world, &call.key),
      reroute: issue
        .and_then(|issue| issue.pending.as_ref())
        .is_some_and(|pending| pending.reroute),
      env: engine.env.clone(),
      paths: engine.paths.clone(),
      reach: engine.reach.clone(),
    }),
    Action::Cleanup => crate::effects_cleanup::job(crate::effects_cleanup::CleanupJob {
      state_dir: issue
        .map(|issue| PathBuf::from(&issue.state_dir))
        .unwrap_or_default(),
      key: call.key.clone(),
      env: engine.env.clone(),
      paths: engine.paths.clone(),
      reach: engine.reach.clone(),
    }),
    Action::Refresh | Action::Close => Box::new(|| "failed: no such epic".to_owned()),
    // Worker prompts go through `attention_ops::send_prompt`, never a call.
    Action::Prompt => Box::new(|| "failed: a prompt is no effect".to_owned()),
    Action::Merge | Action::Reconcile => {
      let epic = issue.map(|issue| issue.epic.clone()).unwrap_or_default();
      let body = json!({"key": call.key, "action": call.action.name(), "epic": epic}).to_string();
      let (scripts, action) = (engine.scripts.clone(), call.action);
      Box::new(move || invoke(scripts.as_deref(), action, &body))
    }
  }
}

/// What an epic action needs from the engine.
fn epic_job(engine: &Engine, epic: &crate::epics::Epic) -> crate::effects_epic::EpicJob {
  crate::effects_epic::EpicJob {
    state_dir: PathBuf::from(&epic.state_dir),
    reference: epic.reference.clone(),
    tracker: epic.tracker.clone(),
    default_repo: epic.default_repo.clone().unwrap_or_default(),
    max_parallel: u64::from(epic.max_parallel),
    recovered: epic
      .pending
      .as_ref()
      .is_some_and(|pending| pending.recovered),
    env: engine.env.clone(),
    paths: engine.paths.clone(),
    reach: engine.reach.clone(),
  }
}

/// Snapshot the worktree the issue's launch record names.
fn checkpoint_job(state: Option<PathBuf>, key: String, trace: Option<PathBuf>) -> Job {
  Box::new(move || {
    let shots = state
      .map(|state| snapshot_active(&state, Some(&key), trace.as_deref()))
      .unwrap_or_default();
    if shots.iter().any(crate::checkpoint::Snapshot::failed) {
      "failed"
    } else {
      "applied"
    }
    .to_owned()
  })
}

#[cfg(test)]
#[path = "tests/effects_run_test.rs"]
mod tests;
