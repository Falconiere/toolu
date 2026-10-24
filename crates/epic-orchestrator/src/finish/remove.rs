//! The three teardown paths (`finish-issue.ts`): a live worktree (exit the
//! agent, verify its workload is gone, snapshot, record the removal intent,
//! remove), a crash after a prepared removal, and a removal already recorded.

use std::path::Path;
use std::time::{Duration, SystemTime};

use serde_json::{Value, json};
use toolu_runtime::process::{Spec, run};

use super::STILL_EXITING;
use super::branch::delete_branch;
use super::ownership::{Cleanup, verify_no_jobs};
use super::record::{RemovalIntent, leftover_files};
use crate::checkpoint::snapshot;
use crate::herdr::Call;
use crate::hosts::host_kind;
use crate::launch::Ctx;
use crate::launch::lifecycle::{
  Expected, Outcome, exit_grace, exit_wait, inspect_agent, listing_gone, shutdown_agent,
};
use crate::launch::record::{Record, text, write_record};
use crate::workload::{OwnedWorkload, await_owned_exit, owned_workload, verify_owned_exit};

const LEFTOVER_LINES: usize = 20;

/// What a teardown proved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Removal {
  /// The worktree is gone.
  pub removed: bool,
  /// The local branch is gone.
  pub branch_deleted: bool,
  /// The snapshot ref of the work.
  pub wip_ref: Option<String>,
  /// Uncommitted files at teardown.
  pub dirty: Vec<String>,
}

/// One teardown: the record file, whether it abandons, and its warnings.
pub(crate) struct Teardown<'a> {
  /// `issues/<key>.json`.
  pub path: &'a Path,
  /// `--abandon` (keeps the branch, refuses dirty work it cannot snapshot).
  pub abandon: bool,
  /// Warnings for the operator.
  pub warnings: Vec<String>,
}

fn word(record: &Record, key: &str) -> String {
  text(record, key).unwrap_or("").to_owned()
}

/// The pids of `list`, comma-separated.
fn pids(list: &[OwnedWorkload]) -> String {
  let pids: Vec<String> = list.iter().map(|owned| owned.pid.to_string()).collect();
  pids.join(", ")
}

/// Exit the agent; babysit's cron is session-scoped, so it ends too. A
/// shutdown that did not finish says whether the agent or its workload is
/// still exiting.
fn exit_agent(ctx: &mut Ctx<'_>, record: &Record, shell: Option<u32>) -> Result<(), String> {
  let agent = word(record, "agent");
  if agent.is_empty() {
    return Ok(());
  }
  let (pane, worktree) = (word(record, "pane_id"), word(record, "worktree"));
  if pane.is_empty() || worktree.is_empty() {
    return Err("cannot verify agent ownership without pane and worktree".to_owned());
  }
  let kind = host_kind(text(record, "kind").unwrap_or("claude"))?;
  let expected = Expected {
    kind,
    pane: &pane,
    cwd: &worktree,
  };
  let outcome = shutdown_agent(&mut ctx.herdr, &agent, &expected).map_err(|err| err.message)?;
  if outcome == Outcome::Exited {
    return Ok(());
  }
  if !listing_gone(&mut ctx.herdr, &agent).map_err(|err| err.message)? {
    let wait = exit_wait(ctx.env).as_secs();
    return Err(format!(
      "{STILL_EXITING}agent {agent} did not leave herdr within {wait} s"
    ));
  }
  let left = owned_workload(Path::new(&worktree), shell)?;
  if left.is_empty() {
    return Ok(());
  }
  Err(format!(
    "{STILL_EXITING}worktree processes {} remain after agent {agent} left herdr",
    pids(&left)
  ))
}

/// The snapshot ref when one holds the worktree's work.
fn snapshot_ref(worktree: &str, key: &str) -> Result<Option<String>, String> {
  let shot = snapshot(worktree, key, None);
  match shot.skipped.as_deref() {
    Some(reason) if reason != "nothing at risk" => {
      Err(format!("required checkpoint failed: {reason}"))
    }
    _ => Ok((shot.changed || shot.sha.is_some()).then_some(shot.reference)),
  }
}

/// The first `git status --porcelain` lines, status columns kept.
fn leftovers(worktree: &str, ctx: &Ctx<'_>) -> Result<Vec<String>, String> {
  if !Path::new(worktree).is_dir() {
    return Ok(Vec::new());
  }
  let mut spec = Spec::new(["git", "-C", worktree, "status", "--porcelain"]);
  spec.env = Some(ctx.env.clone());
  spec.timeout = Duration::from_secs(30);
  let out = run(&spec)
    .ok()
    .filter(|out| out.exit_code == 0 && !out.timed_out && !out.truncated);
  let out = out.ok_or_else(|| format!("cannot inspect worktree {worktree}"))?;
  Ok(
    out
      .stdout
      .lines()
      .filter(|line| !line.is_empty())
      .take(LEFTOVER_LINES)
      .map(str::to_owned)
      .collect(),
  )
}

fn shell_pid(ctx: &mut Ctx<'_>, record: &Record) -> Result<Option<u32>, String> {
  let Some(pane) = text(record, "pane_id") else {
    return Ok(None);
  };
  let info = ctx
    .herdr
    .call(&Call::PaneInfo {
      pane_id: pane.to_owned(),
    })
    .map_err(|err| err.message)?;
  let pid = info
    .pointer("/process_info/shell_pid")
    .and_then(Value::as_u64)
    .and_then(|pid| u32::try_from(pid).ok());
  pid
    .map(Some)
    .ok_or_else(|| "pane shell ownership unavailable".to_owned())
}

impl Teardown<'_> {
  /// Tear down a live worktree.
  ///
  /// # Errors
  /// Workload or jobs remain, dirty work could not be snapshotted, or herdr fails.
  pub(crate) fn live(
    &mut self,
    ctx: &mut Ctx<'_>,
    record: &mut Record,
    ownership: Option<&Cleanup>,
  ) -> Result<Removal, String> {
    let worktree = word(record, "worktree");
    let shell = shell_pid(ctx, record)?;
    let owned = owned_workload(Path::new(&worktree), shell)?;
    exit_agent(ctx, record, shell)?;
    if !await_owned_exit(Path::new(&worktree), shell, &owned, exit_grace(ctx.env))? {
      let now = owned_workload(Path::new(&worktree), shell)?;
      let left = if now.is_empty() { &owned } else { &now };
      return Err(format!(
        "{STILL_EXITING}owned background workload remains alive: {}",
        pids(left)
      ));
    }
    verify_no_jobs(ownership, &worktree)?;
    let wip_ref = snapshot_ref(&worktree, &word(record, "key"))?;
    let dirty = leftovers(&worktree, ctx)?;
    if wip_ref.is_none() && !dirty.is_empty() {
      let what = if self.abandon { "--abandon" } else { "cleanup" };
      return Err(format!(
        "refusing {what}: {worktree} has uncommitted work and the snapshot failed"
      ));
    }
    if !owned_workload(Path::new(&worktree), shell)?.is_empty() {
      return Err(format!(
        "{STILL_EXITING}new workload appeared before cleanup; retry after it exits"
      ));
    }
    let intent = RemovalIntent {
      workspace_id: word(record, "workspace_id"),
      worktree,
      wip_ref,
      leftover_files: dirty,
      workload: owned,
    };
    self.remove_workspace(ctx, record, intent)
  }

  fn remove_workspace(
    &mut self,
    ctx: &mut Ctx<'_>,
    record: &mut Record,
    intent: RemovalIntent,
  ) -> Result<Removal, String> {
    if intent.workspace_id.is_empty() {
      self.warnings.push(format!(
        "WARN: no workspace_id on record {}; skipping worktree remove",
        word(record, "key")
      ));
      let branch_deleted = delete_branch(record, false, self.abandon, ctx.env)?;
      return Ok(Removal {
        removed: false,
        branch_deleted,
        wip_ref: intent.wip_ref,
        dirty: intent.leftover_files,
      });
    }
    let prepared_at = toolu_state::time::iso_millis(SystemTime::now());
    record.insert("stage".to_owned(), json!("cleanup-incomplete"));
    record.insert("removal_intent".to_owned(), intent.to_value(&prepared_at));
    write_record(self.path, record)?;
    let remove = Call::Remove {
      workspace_id: intent.workspace_id.clone(),
      force: !self.abandon,
    };
    match ctx.herdr.call(&remove) {
      Ok(_) => self.removed(ctx, record, intent.wip_ref, intent.leftover_files),
      Err(err) => {
        self.warnings.push(err.message);
        let branch_deleted = delete_branch(record, false, self.abandon, ctx.env)?;
        Ok(Removal {
          removed: false,
          branch_deleted,
          wip_ref: intent.wip_ref,
          dirty: intent.leftover_files,
        })
      }
    }
  }

  /// Persist the removal boundary before touching the branch.
  ///
  /// # Errors
  /// The record cannot be written or git cannot verify the branch.
  pub(crate) fn removed(
    &self,
    ctx: &Ctx<'_>,
    record: &mut Record,
    wip_ref: Option<String>,
    dirty: Vec<String>,
  ) -> Result<Removal, String> {
    for (key, value) in [
      ("stage", json!("cleanup-incomplete")),
      ("worktree_removed", json!(true)),
      ("branch_deleted", json!(false)),
      ("wip_ref", json!(wip_ref)),
      ("leftover_files", json!(dirty)),
    ] {
      record.insert(key.to_owned(), value);
    }
    write_record(self.path, record)?;
    let branch_deleted = delete_branch(record, true, self.abandon, ctx.env)?;
    Ok(Removal {
      removed: true,
      branch_deleted,
      wip_ref,
      dirty,
    })
  }

  /// A removal recorded before: only the branch may remain.
  ///
  /// # Errors
  /// Jobs remain, or git cannot verify the branch.
  pub(crate) fn after(
    &self,
    ctx: &Ctx<'_>,
    record: &Record,
    ownership: Option<&Cleanup>,
  ) -> Result<Removal, String> {
    verify_no_jobs(ownership, text(record, "worktree").unwrap_or(""))?;
    Ok(Removal {
      removed: true,
      branch_deleted: delete_branch(record, true, self.abandon, ctx.env)?,
      wip_ref: text(record, "wip_ref").map(str::to_owned),
      dirty: leftover_files(record),
    })
  }
}

/// Verify a crash after a prepared removal left nothing behind.
///
/// # Errors
/// The intent is another workspace's, the agent or workload lives, jobs
/// remain, or the worktree is still registered.
pub(crate) fn verify_prepared(
  ctx: &mut Ctx<'_>,
  record: &Record,
  intent: &RemovalIntent,
  ownership: &Cleanup,
) -> Result<(), String> {
  if Some(intent.worktree.as_str()) != text(record, "worktree")
    || Some(intent.workspace_id.as_str()) != text(record, "workspace_id")
  {
    return Err("removal intent does not match recorded workspace ownership".to_owned());
  }
  let agent = word(record, "agent");
  if !agent.is_empty()
    && inspect_agent(&mut ctx.herdr, &agent)
      .map_err(|err| err.message)?
      .is_some()
  {
    return Err(format!(
      "agent {agent} is still live after worktree removal"
    ));
  }
  if !verify_owned_exit(&intent.workload)? {
    return Err("owned background workload remains alive after worktree removal".to_owned());
  }
  verify_no_jobs(Some(ownership), &intent.worktree)?;
  let mut spec = Spec::new([
    "git",
    "-C",
    &word(record, "checkout"),
    "worktree",
    "list",
    "--porcelain",
    "-z",
  ]);
  spec.env = Some(ctx.env.clone());
  spec.timeout = Duration::from_secs(30);
  let listed = run(&spec)
    .ok()
    .filter(|out| out.exit_code == 0 && !out.timed_out && !out.truncated);
  let listed = listed.ok_or_else(|| "cannot verify removed worktree registration".to_owned())?;
  let registered = listed
    .stdout
    .split('\0')
    .filter_map(|line| line.strip_prefix("worktree "))
    .any(|path| Path::new(path) == Path::new(&intent.worktree));
  if registered {
    return Err("worktree path is absent but remains registered".to_owned());
  }
  Ok(())
}

#[cfg(test)]
#[path = "tests/remove_test.rs"]
mod tests;
