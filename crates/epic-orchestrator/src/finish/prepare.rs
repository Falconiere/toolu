//! Prepare a live worktree for removal: exit the agent, snapshot its work,
//! and list what is left behind.

use std::path::Path;
use std::time::Duration;

use serde_json::Value;
use toolu_runtime::process::{Spec, run};

use super::STILL_EXITING;
use crate::checkpoint::snapshot;
use crate::herdr::Call;
use crate::hosts::host_kind;
use crate::launch::Ctx;
use crate::launch::lifecycle::{
  Expected, Outcome, exit_grace, exit_wait, listing_gone, shutdown_agent,
};
use crate::launch::record::{Record, text};
use crate::workload::{OwnedWorkload, await_owned_exit, owned_workload};

const LEFTOVER_LINES: usize = 20;

pub(crate) fn word(record: &Record, key: &str) -> String {
  text(record, key).unwrap_or("").to_owned()
}

/// The pids of `list`, comma-separated.
pub(crate) fn pids(list: &[OwnedWorkload]) -> String {
  let pids: Vec<String> = list.iter().map(|owned| owned.pid.to_string()).collect();
  pids.join(", ")
}

/// Exit the agent; babysit's cron is session-scoped, so it ends too. A
/// shutdown that did not finish says whether the agent or its workload is
/// still exiting.
pub(crate) fn exit_agent(
  ctx: &mut Ctx<'_>,
  record: &Record,
  shell: Option<u32>,
) -> Result<(), String> {
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
  // Nothing owned now; `Teardown::live` still awaits the set it recorded
  // before the exit, so a process that moved out of the worktree is caught.
  if left.is_empty() {
    return Ok(());
  }
  Err(format!(
    "{STILL_EXITING}worktree processes {} remain after agent {agent} left herdr",
    pids(&left)
  ))
}

/// The snapshot ref when one holds the worktree's work.
pub(crate) fn snapshot_ref(worktree: &str, key: &str) -> Result<Option<String>, String> {
  let shot = snapshot(worktree, key, None);
  match shot.skipped.as_deref() {
    Some(reason) if reason != "nothing at risk" => {
      Err(format!("required checkpoint failed: {reason}"))
    }
    _ => Ok((shot.changed || shot.sha.is_some()).then_some(shot.reference)),
  }
}

/// The first `git status --porcelain` lines, status columns kept.
pub(crate) fn leftovers(worktree: &str, ctx: &Ctx<'_>) -> Result<Vec<String>, String> {
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

/// Wait for the recorded workload to leave, unless the worktree is gone and
/// holds nothing to wait for.
///
/// # Errors
/// Owned background workload remains alive.
pub(crate) fn await_quiet(
  ctx: &Ctx<'_>,
  worktree: &str,
  shell: Option<u32>,
  owned: &[OwnedWorkload],
  absent: bool,
) -> Result<(), String> {
  if absent || await_owned_exit(Path::new(worktree), shell, owned, exit_grace(ctx.env))? {
    return Ok(());
  }
  let now = owned_workload(Path::new(worktree), shell)?;
  let left = if now.is_empty() { owned } else { &now };
  Err(format!(
    "{STILL_EXITING}owned background workload remains alive: {}",
    pids(left)
  ))
}

/// Snapshot the work and list leftovers; an absent worktree records neither.
/// Refuses dirty work no snapshot holds.
///
/// # Errors
/// Dirty work could not be snapshotted.
pub(crate) fn capture_work(
  ctx: &Ctx<'_>,
  record: &Record,
  worktree: &str,
  absent: bool,
  abandon: bool,
) -> Result<(Option<String>, Vec<String>), String> {
  // `snapshot_ref` fails on the "no worktree" skip, so an absent worktree
  // records no ref directly; `leftovers` already tolerates absence.
  let wip_ref = if absent {
    None
  } else {
    snapshot_ref(worktree, &word(record, "key"))?
  };
  let dirty = leftovers(worktree, ctx)?;
  if wip_ref.is_none() && !dirty.is_empty() {
    let what = if abandon { "--abandon" } else { "cleanup" };
    return Err(format!(
      "refusing {what}: {worktree} has uncommitted work and the snapshot failed"
    ));
  }
  Ok((wip_ref, dirty))
}

/// The pane's shell pid, if herdr still knows the pane. A pane herdr no
/// longer knows means there is no shell to verify (#633).
pub(crate) fn shell_pid(ctx: &mut Ctx<'_>, record: &Record) -> Result<Option<u32>, String> {
  let Some(pane) = text(record, "pane_id") else {
    return Ok(None);
  };
  let info = match ctx.herdr.call(&Call::PaneInfo {
    pane_id: pane.to_owned(),
  }) {
    Err(err) if err.code.as_deref() == Some("pane_not_found") => return Ok(None),
    result => result.map_err(|err| err.message)?,
  };
  let pid = info
    .pointer("/process_info/shell_pid")
    .and_then(Value::as_u64)
    .and_then(|pid| u32::try_from(pid).ok());
  pid
    .map(Some)
    .ok_or_else(|| "pane shell ownership unavailable".to_owned())
}

#[cfg(test)]
#[path = "tests/prepare_test.rs"]
mod tests;
