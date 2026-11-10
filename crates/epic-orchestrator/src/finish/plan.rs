//! Dry-run teardown plans.

use super::{FinishReport, never_started, read_issue_record, require_merged};
use crate::launch::Ctx;
use crate::launch::record::{Record, text};
use serde_json::json;
use std::path::Path;

/// Validate a local teardown and describe its actions without changing state.
pub(super) fn plan(
  state_dir: &Path,
  key: &str,
  abandon: bool,
  ctx: &Ctx<'_>,
) -> Result<FinishReport, String> {
  let path = state_dir.join("issues").join(format!("{key}.json"));
  let record = read_issue_record(&path, key)?;
  let word = |field: &str| text(&record, field).unwrap_or("").to_owned();
  let abandon = abandon || word("stage") == "abandoned";
  if !abandon {
    require_merged(state_dir, key, &word("repo"), ctx.gh.clone())?;
  }
  if abandon && never_started(&record) {
    let lease = text(&record, "lease_token").map(|token| format!("release lease {token}"));
    let finished = json!({"dry_run": true, "key": key, "stage": word("stage"), "actions": lease.into_iter().collect::<Vec<_>>()});
    return Ok(FinishReport {
      finished,
      warnings: Vec::new(),
    });
  }
  let actions = plan_actions(&record, key, abandon);
  let finished = json!({"dry_run": true, "key": key, "stage": word("stage"), "actions": actions});
  Ok(FinishReport {
    finished,
    warnings: Vec::new(),
  })
}

fn plan_actions(record: &Record, key: &str, abandon: bool) -> Vec<String> {
  let word = |field: &str| text(record, field).unwrap_or("").to_owned();
  let mut actions = Vec::new();
  if !word("agent").is_empty() {
    actions.push(format!("herdr agent prompt {} /exit", word("agent")));
  }
  actions.push(format!(
    "snapshot {} at refs/epic-wip/{key}",
    word("worktree")
  ));
  if !word("workspace_id").is_empty() {
    let force = if abandon { "" } else { " --force" };
    actions.push(format!(
      "herdr worktree remove --workspace {}{force}",
      word("workspace_id")
    ));
  }
  if !abandon {
    actions.push(format!(
      "git -C {} branch -D {}",
      word("checkout"),
      word("branch")
    ));
  }
  actions
}

#[cfg(test)]
#[path = "tests/plan_test.rs"]
mod tests;
