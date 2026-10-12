//! Build the in-process merge-queue step (#448) for an issue on the state
//! thread: the head is worked while its worker is parked, an issue that left
//! the queue only has its auto-merge disarmed, and a recovered step asks
//! GitHub whether the merge already happened.

use std::path::PathBuf;

use serde_json::Value;

use crate::executor::Job;
use crate::model::{Action, Issue};
use crate::queue_step::{Mode, StepJob};
use crate::rebase::Lease;
use crate::server::Engine;

/// The effect body of `action` (merge or its reconcile) for `key`.
pub(crate) fn job(engine: &Engine, key: &str, action: Action) -> Job {
  let Some(issue) = engine.world.issues.get(key) else {
    return Box::new(|| "failed: no such issue".to_owned());
  };
  let Some(pr) = issue.pr.filter(|_| !issue.repo.is_empty()) else {
    return Box::new(|| "failed: no pull request".to_owned());
  };
  let mode = if crate::queue::is_head(&engine.world, key) {
    Mode::Head {
      parked: parked(engine, issue),
    }
  } else {
    Mode::Disarm
  };
  let record = crate::disk::read_value(&crate::snapshot::issue_path(&issue.state_dir, key))
    .unwrap_or(Value::Null);
  let input = StepJob {
    repo: issue.repo.clone(),
    pr,
    key: key.to_owned(),
    mode,
    worktree: record
      .get("worktree")
      .and_then(Value::as_str)
      .map(str::to_owned)
      .or_else(|| issue.worktree.clone())
      .map(PathBuf::from),
    lease: lease(&record),
    env: engine.env.clone(),
    reach: engine.reach.clone(),
    trace: engine.git_trace.clone(),
  };
  if action == Action::Reconcile {
    return Box::new(move || crate::queue_step::reconcile(&input));
  }
  crate::queue_step::job(input)
}

/// The worker may be rebased under: `ready`, not working, herdr reachable.
fn parked(engine: &Engine, issue: &Issue) -> bool {
  issue.phase == "ready" && !issue.agent_working && engine.world.herdr_failures == 0
}

/// The agent lease the launch recorded.
fn lease(record: &Value) -> Option<Lease> {
  let text = |name: &str| record.get(name).and_then(Value::as_str).filter(|value| !value.is_empty());
  Some(Lease {
    root: PathBuf::from(text("resource_root")?),
    token: text("lease_token")?.to_owned(),
  })
}

#[cfg(test)]
#[path = "tests/effects_queue_test.rs"]
mod tests;
