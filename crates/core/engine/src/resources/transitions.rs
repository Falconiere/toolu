//! Ownership transitions that never release capacity between states
//! (`packages/toolu-core/src/resources/lease-transitions.ts`): an agent moves
//! to another host only after its destination is reserved and its old jobs
//! are gone, and cleanup fences a worktree before shutdown so no new job
//! starts in it.

use std::fmt;
use std::path::Path;
use std::time::SystemTime;

use toolu_runtime::json::ordered::Ordered;
use toolu_state::time::iso_millis;

use super::leases::{admit_pressure, cooling, count, reconcile_jobs, text};
use super::store::{leases, leases_mut, resource_policy, update_resources};
use crate::ledger::jq::string;

/// The destination caps of an agent migration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MigrationCaps {
  /// The destination host's cap for this run.
  pub host_cap: i64,
  /// The epic's agent cap.
  pub epic_cap: Option<i64>,
}

/// Why a worktree fence was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FenceError {
  /// A managed job still runs in the worktree; the lease is `cleanup-incomplete`.
  JobsActive,
  /// Any other refusal.
  Failed(String),
}

impl fmt::Display for FenceError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::JobsActive => f.write_str("active jobs prevent worktree cleanup"),
      Self::Failed(reason) => f.write_str(reason),
    }
  }
}

fn agent_mut<'a>(state: &'a mut Ordered, token: &str) -> Result<&'a mut Ordered, String> {
  leases_mut(state)
    .and_then(|items| {
      items
        .iter_mut()
        .find(|lease| text(lease, "token") == Some(token))
    })
    .filter(|lease| text(lease, "type") == Some("agent"))
    .ok_or_else(|| "agent resource lease lost".to_owned())
}

fn remove(lease: &mut Ordered, key: &str) {
  if let Ordered::Object(fields) = lease {
    fields.retain(|(name, _)| name != key);
  }
}

fn worktree_job(state: &Ordered, worktree: &str) -> bool {
  leases(state)
    .iter()
    .any(|lease| text(lease, "type") == Some("job") && text(lease, "worktree") == Some(worktree))
}

fn validate(target: &str, caps: MigrationCaps) -> Result<(), String> {
  if target.is_empty() {
    return Err("invalid target host".to_owned());
  }
  if caps.host_cap <= 0 {
    return Err("invalid host capacity".to_owned());
  }
  if caps.epic_cap.is_some_and(|cap| cap <= 0) {
    return Err("invalid epic capacity".to_owned());
  }
  Ok(())
}

/// One requested move.
struct Move<'a> {
  token: &'a str,
  target: &'a str,
  caps: MigrationCaps,
}

/// Machine, epic and destination-host capacity without the moving lease.
fn reserve(
  root: &Path,
  state: &mut Ordered,
  wanted: &Move<'_>,
  now: SystemTime,
) -> Result<(), String> {
  let Move {
    token,
    target,
    caps,
  } = *wanted;
  let policy = resource_policy(root)?;
  admit_pressure(state, &policy, now)?;
  let moving_epic = leases(state)
    .iter()
    .find(|lease| text(lease, "token") == Some(token))
    .and_then(|lease| text(lease, "stateDir"))
    .map(str::to_owned);
  let others: Vec<&Ordered> = leases(state)
    .iter()
    .filter(|lease| text(lease, "type") == Some("agent") && text(lease, "token") != Some(token))
    .collect();
  if count(others.len()) >= policy.max_agents {
    return Err(format!("agent capacity exhausted ({})", policy.max_agents));
  }
  let in_epic = others
    .iter()
    .filter(|lease| text(lease, "stateDir") == moving_epic.as_deref())
    .count();
  if caps.epic_cap.is_some_and(|cap| count(in_epic) >= cap) {
    return Err("epic capacity exhausted".to_owned());
  }
  let configured = policy
    .hosts
    .iter()
    .find(|(name, _)| name == target)
    .map_or(policy.max_agents, |(_, cap)| *cap);
  if cooling(state, target, now) {
    return Err(format!("{target} is cooling down"));
  }
  let on_target = others
    .iter()
    .filter(|lease| {
      text(lease, "host") == Some(target) || text(lease, "pendingHost") == Some(target)
    })
    .count();
  if count(on_target) >= caps.host_cap.min(configured) {
    return Err(format!("{target} capacity exhausted"));
  }
  Ok(())
}

/// `prepareAgentMigration`: reserve `target` on the source lease (`pendingHost`)
/// before anything stops the source agent.
///
/// # Errors
/// Invalid caps, a lost lease, another reservation, a hold, exhausted capacity
/// or a cooling destination.
pub fn prepare_agent_migration(
  root: &Path,
  token: &str,
  target: &str,
  caps: MigrationCaps,
  now: SystemTime,
) -> Result<Ordered, String> {
  validate(target, caps)?;
  update_resources(root, |state| {
    let pending = text(agent_mut(state, token)?, "pendingHost").map(str::to_owned);
    match pending.as_deref() {
      Some(host) if host == target => {}
      Some(_) => return Err("agent migration already reserved".to_owned()),
      None => {
        reserve(
          root,
          state,
          &Move {
            token,
            target,
            caps,
          },
          now,
        )?;
        agent_mut(state, token)?.set("pendingHost", string(target));
      }
    }
    let lease = agent_mut(state, token)?;
    lease.set("stage", string("replacing"));
    lease.set("heartbeatAt", string(&iso_millis(now)));
    Ok(lease.clone())
  })
}

/// `migrateAgentLease`: move the lease to its reserved `target` once no job of
/// its worktree remains.
///
/// # Errors
/// A lost lease, no reservation for `target`, or a live worktree job.
pub fn migrate_agent_lease(
  root: &Path,
  token: &str,
  target: &str,
  now: SystemTime,
) -> Result<Ordered, String> {
  update_resources(root, |state| {
    let lease = agent_mut(state, token)?;
    if text(lease, "stage") != Some("replacing") || text(lease, "pendingHost") != Some(target) {
      return Err("agent target capacity is not reserved".to_owned());
    }
    let worktree = text(lease, "worktree").map(str::to_owned);
    reconcile_jobs(state)?;
    if worktree
      .as_deref()
      .is_some_and(|tree| worktree_job(state, tree))
    {
      return Err("active jobs prevent agent migration".to_owned());
    }
    let lease = agent_mut(state, token)?;
    lease.set("host", string(target));
    remove(lease, "pendingHost");
    lease.set("heartbeatAt", string(&iso_millis(now)));
    Ok(lease.clone())
  })
}

/// `cancelAgentMigration`: drop an unused reservation; the source runs on.
///
/// # Errors
/// A lost lease or no reservation.
pub fn cancel_agent_migration(
  root: &Path,
  token: &str,
  now: SystemTime,
) -> Result<Ordered, String> {
  update_resources(root, |state| {
    let lease = agent_mut(state, token)?;
    if text(lease, "pendingHost").is_none() {
      return Err("agent migration is not reserved".to_owned());
    }
    remove(lease, "pendingHost");
    lease.set("stage", string("running"));
    lease.set("heartbeatAt", string(&iso_millis(now)));
    Ok(lease.clone())
  })
}

/// `fenceWorktree`: persist the cleanup fence (`cleaning`) so new jobs in
/// `worktree` are refused; a live job leaves the lease `cleanup-incomplete`.
///
/// # Errors
/// [`FenceError::JobsActive`] while a job runs there, or a lost or mismatched lease.
pub fn fence_worktree(
  root: &Path,
  token: &str,
  worktree: &str,
  now: SystemTime,
) -> Result<Ordered, FenceError> {
  if worktree.is_empty() {
    return Err(FenceError::Failed("invalid fenced worktree".to_owned()));
  }
  let mut active = false;
  let outcome = update_resources(root, |state| {
    let recorded = text(agent_mut(state, token)?, "worktree").map(str::to_owned);
    if recorded.as_deref().is_some_and(|tree| tree != worktree) {
      return Err("agent lease worktree does not match cleanup target".to_owned());
    }
    reconcile_jobs(state)?;
    active = worktree_job(state, worktree);
    let lease = agent_mut(state, token)?;
    lease.set("worktree", string(worktree));
    lease.set(
      "stage",
      string(if active {
        "cleanup-incomplete"
      } else {
        "cleaning"
      }),
    );
    lease.set("heartbeatAt", string(&iso_millis(now)));
    Ok(lease.clone())
  });
  match outcome {
    Ok(_) if active => Err(FenceError::JobsActive),
    Ok(lease) => Ok(lease),
    Err(reason) => Err(FenceError::Failed(reason)),
  }
}

/// `releaseLease(root, token)`: an agent's lease stays while a job of its worktree runs.
///
/// # Errors
/// `active jobs prevent agent lease release`, or a lock, state or probe failure.
pub fn release_lease(root: &Path, token: &str) -> Result<(), String> {
  update_resources(root, |state| {
    reconcile_jobs(state)?;
    let worktree = leases(state)
      .iter()
      .find(|l| text(l, "token") == Some(token) && text(l, "type") == Some("agent"))
      .and_then(|l| text(l, "worktree"));
    if worktree.is_some_and(|tree| worktree_job(state, tree)) {
      return Err("active jobs prevent agent lease release".to_owned());
    }
    if let Some(items) = leases_mut(state) {
      items.retain(|l| text(l, "token") != Some(token));
    }
    Ok(())
  })
}

/// `activeJobs`: a managed job lease holds `worktree`.
///
/// # Errors
/// The resource state cannot be read.
pub fn active_jobs(root: &Path, worktree: &str) -> Result<bool, String> {
  let state = super::store::read_resource_state(root)?;
  Ok(worktree_job(&state, worktree))
}

#[cfg(test)]
#[path = "tests/transitions_rig_test.rs"]
mod rig;

#[cfg(test)]
#[path = "tests/transitions_test.rs"]
mod tests;
