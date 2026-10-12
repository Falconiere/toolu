//! Tear down one sub-issue after its PR merged (`finish-issue.ts`): exit the
//! agent, snapshot and remove the herdr worktree workspace, delete the local
//! branch, mark the record merged and release its lease. `--abandon` skips
//! the merge check, keeps the branch, and refuses dirty work it could not
//! snapshot. A teardown step that fails leaves `cleanup-incomplete` and the
//! lease; a missing or unmerged PR refuses before anything is written.

use std::path::Path;
use std::time::SystemTime;

use serde_json::{Value, json};

mod branch;
mod ownership;
mod record;
mod remove;

use ownership::{fence_cleanup_ownership, release_recorded_ownership};
use record::{finished, read_issue_record, removal_intent, terminal_cleanup_complete};
use remove::{Removal, Teardown, verify_prepared};

use crate::launch::Ctx;
use crate::launch::Failure;
use crate::launch::admission::with_launch_ownership;
use crate::launch::record::{Record, text, write_record};
use crate::tracker::Gh;

/// What a finish printed, and its warnings for stderr.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FinishReport {
  /// The finish document.
  pub finished: Value,
  /// Warnings for the operator.
  pub warnings: Vec<String>,
}

/// Refuse unless the PR the worker reported is merged.
fn require_merged(
  state_dir: &Path,
  key: &str,
  repo: &str,
  gh: Result<&Gh, String>,
) -> Result<(), String> {
  let status = crate::disk::read_value(&state_dir.join("status").join(format!("{key}.json")))
    .unwrap_or(Value::Null);
  let pr = match status.get("pr") {
    Some(Value::Number(number)) => number.to_string(),
    Some(Value::String(number)) if !number.is_empty() => number.clone(),
    _ => {
      return Err(format!(
        "no PR recorded for {key}; use --abandon to tear down anyway"
      ));
    }
  };
  let body: Value = gh.and_then(|gh| gh.get(&format!("/repos/{repo}/pulls/{pr}")))?;
  if body.get("merged").and_then(Value::as_bool) == Some(true) {
    return Ok(());
  }
  let state = body
    .get("state")
    .and_then(Value::as_str)
    .unwrap_or("unknown")
    .to_uppercase();
  Err(format!("PR {repo}#{pr} is {state}, not MERGED"))
}

/// Finish `key` under the per-issue launch lock (a dry run plans only).
///
/// # Errors
/// A teardown failure, which leaves the record `cleanup-incomplete` with the
/// reason; a missing or unmerged PR refuses without writing.
pub(crate) fn finish_issue(
  state_dir: &Path,
  key: &str,
  flags: (bool, bool),
  ctx: &mut Ctx<'_>,
) -> Result<FinishReport, String> {
  let (abandon, dry_run) = flags;
  if dry_run {
    return plan(state_dir, key, abandon, ctx);
  }
  with_launch_ownership(state_dir, key, || {
    finish(state_dir, key, abandon, ctx).map_err(Failure::unknown)
  })
  .map_err(|err| err.message)
}

fn plan(state_dir: &Path, key: &str, abandon: bool, ctx: &Ctx<'_>) -> Result<FinishReport, String> {
  let path = state_dir.join("issues").join(format!("{key}.json"));
  let record = read_issue_record(&path, key)?;
  let word = |field: &str| text(&record, field).unwrap_or("").to_owned();
  let abandon = abandon || word("stage") == "abandoned";
  if !abandon {
    require_merged(state_dir, key, &word("repo"), ctx.gh.clone())?;
  }
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
  let finished = json!({"dry_run": true, "key": key, "stage": word("stage"), "actions": actions});
  Ok(FinishReport {
    finished,
    warnings: Vec::new(),
  })
}

fn finish(
  state_dir: &Path,
  key: &str,
  abandon: bool,
  ctx: &mut Ctx<'_>,
) -> Result<FinishReport, String> {
  let path = state_dir.join("issues").join(format!("{key}.json"));
  let dir = state_dir.display().to_string();
  let mut record = read_issue_record(&path, key)?;
  let stage = text(&record, "stage").unwrap_or("").to_owned();
  let terminal = matches!(stage.as_str(), "merged" | "abandoned");
  if terminal && terminal_cleanup_complete(&record) {
    release_recorded_ownership(&record, &dir)?;
    return Ok(FinishReport {
      finished: finished(&record),
      warnings: Vec::new(),
    });
  }
  let abandon = abandon || stage == "abandoned";
  precheck(
    state_dir,
    key,
    &mut record,
    (terminal, abandon),
    ctx.gh.clone(),
  )?;
  let mut teardown = Teardown {
    path: &path,
    abandon,
    warnings: Vec::new(),
  };
  let removal = match tear_down(ctx, &mut teardown, &dir, key, &mut record) {
    Ok(removal) => removal,
    Err(err) => {
      incomplete(&path, &mut record, &err)?;
      return Err(err);
    }
  };
  record_finish(
    &path,
    &dir,
    &mut record,
    (&removal, abandon),
    teardown.warnings,
  )
}

/// Write the teardown's final stage and evidence, and release the lease
/// unless the cleanup is incomplete.
fn record_finish(
  path: &Path,
  dir: &str,
  record: &mut Record,
  (removal, abandon): (&Removal, bool),
  warnings: Vec<String>,
) -> Result<FinishReport, String> {
  settle(record, removal, abandon);
  write_record(path, record)?;
  if text(record, "stage") != Some("cleanup-incomplete") {
    release_recorded_ownership(record, dir)?;
  }
  Ok(FinishReport {
    finished: finished(record),
    warnings,
  })
}

/// Mark an unverified terminal record incomplete and, unless abandoning,
/// refuse a PR that is not merged.
fn precheck(
  state_dir: &Path,
  key: &str,
  record: &mut Record,
  (terminal, abandon): (bool, bool),
  gh: Result<&Gh, String>,
) -> Result<(), String> {
  let path = state_dir.join("issues").join(format!("{key}.json"));
  if terminal {
    incomplete(
      &path,
      record,
      "terminal record lacks verified cleanup evidence",
    )?;
  }
  if abandon {
    return Ok(());
  }
  require_merged(state_dir, key, text(record, "repo").unwrap_or(""), gh)
}

/// Leave the record `cleanup-incomplete` with `reason`.
fn incomplete(path: &Path, record: &mut Record, reason: &str) -> Result<(), String> {
  record.insert("stage".to_owned(), json!("cleanup-incomplete"));
  record.remove("finished_at");
  record.insert("cleanup_error".to_owned(), json!(reason));
  write_record(path, record)
}

fn tear_down(
  ctx: &mut Ctx<'_>,
  teardown: &mut Teardown<'_>,
  state: &str,
  key: &str,
  record: &mut Record,
) -> Result<Removal, String> {
  let ownership = fence_cleanup_ownership(teardown.path, state, key, record)?;
  let absent = !text(record, "worktree").is_some_and(|path| Path::new(path).exists());
  let intent = removal_intent(record)?;
  let recorded = record.get("worktree_removed") == Some(&Value::Bool(true));
  let removed_before = recorded && absent;
  let prepared = !recorded && absent && intent.is_some();
  match (prepared, removed_before, ownership, intent) {
    (true, _, Some(ownership), Some(intent)) => {
      verify_prepared(ctx, record, &intent, &ownership)?;
      teardown.removed(ctx, record, intent.wip_ref, intent.leftover_files)
    }
    (_, true, Some(ownership), _) => teardown.after(ctx, record, Some(&ownership)),
    (true, _, None, _) | (_, true, None, _) => {
      Err("cannot verify legacy cleanup ownership after worktree removal".to_owned())
    }
    (_, _, ownership, _) => teardown.live(ctx, record, ownership.as_ref()),
  }
}

/// The final stage and evidence of a teardown.
fn settle(record: &mut Record, removal: &Removal, abandon: bool) {
  let done = removal.removed && (abandon || removal.branch_deleted);
  let stage = match (done, abandon) {
    (true, true) => "abandoned",
    (true, false) => "merged",
    (false, _) => "cleanup-incomplete",
  };
  let at = toolu_state::time::iso_seconds(SystemTime::now());
  if done {
    record.insert("finished_at".to_owned(), json!(at));
    record.remove("cleanup_error");
    record.remove("removal_intent");
  } else {
    record.insert("cleanup_attempt_at".to_owned(), json!(at));
    record.remove("finished_at");
  }
  for (key, value) in [
    ("stage", json!(stage)),
    ("worktree_removed", json!(removal.removed)),
    ("branch_deleted", json!(removal.branch_deleted)),
    ("wip_ref", json!(removal.wip_ref)),
    ("leftover_files", json!(removal.dirty)),
  ] {
    record.insert(key.to_owned(), value);
  }
}

#[cfg(test)]
#[path = "tests/finish_test.rs"]
mod tests;
