//! The three teardown paths (`finish-issue.ts`): a live worktree (exit the
//! agent, verify its workload is gone, snapshot, record the removal intent,
//! remove), a crash after a prepared removal, and a removal already recorded.

use std::path::Path;
use std::time::{Duration, SystemTime};

use serde_json::json;
use toolu_runtime::env::Env;
use toolu_runtime::process::{Spec, run};

use super::STILL_EXITING;
use super::branch::delete_branch;
use super::ownership::{Cleanup, verify_no_jobs};
use super::prepare::{await_quiet, capture_work, exit_agent, shell_pid, word};
use super::record::{RemovalIntent, leftover_files};
use crate::herdr::Call;
use crate::launch::Ctx;
use crate::launch::lifecycle::inspect_agent;
use crate::launch::record::{Record, text, write_record};
use crate::workload::{owned_workload, verify_owned_exit};

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

/// Whether git still registers `worktree` under `checkout`.
///
/// # Errors
/// git cannot list its worktrees.
fn worktree_registered(record: &Record, intent: &RemovalIntent, env: &Env) -> Result<bool, String> {
  let mut spec = Spec::new([
    "git",
    "-C",
    &word(record, "checkout"),
    "worktree",
    "list",
    "--porcelain",
    "-z",
  ]);
  spec.env = Some(env.clone());
  spec.timeout = Duration::from_secs(30);
  let listed = run(&spec)
    .ok()
    .filter(|out| out.exit_code == 0 && !out.timed_out && !out.truncated);
  let listed = listed.ok_or_else(|| "cannot verify removed worktree registration".to_owned())?;
  Ok(
    listed
      .stdout
      .split('\0')
      .filter_map(|line| line.strip_prefix("worktree "))
      .any(|path| Path::new(path) == Path::new(&intent.worktree)),
  )
}

/// Remove what a gone workspace left behind (#633): nothing when the path is
/// absent and git no longer registers it, else a git removal so no
/// registration blocks the branch delete.
///
/// # Errors
/// git cannot verify or remove the worktree.
fn remove_gone_worktree(
  record: &Record,
  intent: &RemovalIntent,
  abandon: bool,
  env: &Env,
) -> Result<(), String> {
  if !Path::new(&intent.worktree).exists() && !worktree_registered(record, intent, env)? {
    return Ok(());
  }
  git_remove_worktree(record, intent, abandon, env)
}

/// Remove the worktree with git.
///
/// # Errors
/// git cannot remove the worktree.
fn git_remove_worktree(
  record: &Record,
  intent: &RemovalIntent,
  abandon: bool,
  env: &Env,
) -> Result<(), String> {
  let (checkout, worktree) = (word(record, "checkout"), intent.worktree.clone());
  let mut argv: Vec<&str> = vec!["git", "-C", &checkout, "worktree", "remove"];
  if !abandon {
    argv.push("--force");
  }
  argv.push(&worktree);
  let mut spec = Spec::new(argv);
  spec.env = Some(env.clone());
  spec.timeout = Duration::from_secs(120);
  let out = run(&spec)
    .ok()
    .filter(|out| !out.timed_out && !out.truncated);
  match out {
    Some(out) if out.exit_code == 0 => Ok(()),
    Some(out) => Err(format!(
      "git worktree remove {worktree} failed: {}",
      out.stderr.trim()
    )),
    None => Err(format!("git worktree remove {worktree} failed")),
  }
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
    // A worktree removed by hand holds nothing to own or snapshot (#633).
    let absent = !worktree.is_empty() && !Path::new(&worktree).exists();
    let shell = shell_pid(ctx, record)?;
    let owned = if absent {
      Vec::new()
    } else {
      owned_workload(Path::new(&worktree), shell)?
    };
    exit_agent(ctx, record, shell)?;
    await_quiet(ctx, &worktree, shell, &owned, absent)?;
    verify_no_jobs(ownership, &worktree)?;
    let (wip_ref, dirty) = capture_work(ctx, record, &worktree, absent, self.abandon)?;
    if !absent && !owned_workload(Path::new(&worktree), shell)?.is_empty() {
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
      // The workspace is already gone with the agent and its pane: remove
      // the worktree with git so no registration blocks the branch (#633).
      Err(err) if err.code.as_deref() == Some("workspace_not_found") => {
        match remove_gone_worktree(record, &intent, self.abandon, ctx.env) {
          Ok(()) => self.removed(ctx, record, intent.wip_ref, intent.leftover_files),
          Err(warning) => self.unremoved(ctx, record, intent, warning),
        }
      }
      Err(err) => self.unremoved(ctx, record, intent, err.message),
    }
  }

  /// Keep the record after a removal that did not happen: warn, then delete
  /// only what a kept worktree allows.
  ///
  /// # Errors
  /// git cannot verify the branch.
  fn unremoved(
    &mut self,
    ctx: &Ctx<'_>,
    record: &Record,
    intent: RemovalIntent,
    warning: String,
  ) -> Result<Removal, String> {
    self.warnings.push(warning);
    let branch_deleted = delete_branch(record, false, self.abandon, ctx.env)?;
    Ok(Removal {
      removed: false,
      branch_deleted,
      wip_ref: intent.wip_ref,
      dirty: intent.leftover_files,
    })
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
  if worktree_registered(record, intent, ctx.env)? {
    return Err("worktree path is absent but remains registered".to_owned());
  }
  Ok(())
}

#[cfg(test)]
#[path = "tests/remove_test.rs"]
mod tests;
