//! Machine-wide leases (`packages/toolu-core/src/resources/resources.ts`):
//! admission under capacity, host caps, cooldowns and pressure; the lease's
//! later stages; and reclamation of jobs whose owner and process group are
//! both provably gone. Agent migration and worktree fences stay in TypeScript
//! until the epic engine (#434) adds them here.

use std::path::Path;
use std::time::SystemTime;

use toolu_runtime::json::ordered::Ordered;
use toolu_runtime::process::group;
use toolu_state::lock::token;
use toolu_state::time::{epoch_millis, iso_millis};

use super::lock::{process_alive, whole};
use super::pressure::{PRESSURE_SAMPLE_MS, Pressure, advance_pressure, pressure_of};
use super::sample::sample_resources;
use super::store::{ResourcePolicy, leases, leases_mut, resource_policy, update_resources};
use crate::ledger::jq::{number, string};

/// What a lease is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeaseKind {
  /// A running agent.
  Agent,
  /// An expensive job, such as a plan-ledger check.
  Job,
}

impl LeaseKind {
  fn name(self) -> &'static str {
    match self {
      LeaseKind::Agent => "agent",
      LeaseKind::Job => "job",
    }
  }
}

/// `LeaseRequest`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeaseRequest {
  /// Agent or job.
  pub kind: LeaseKind,
  /// The owner's key.
  pub key: String,
  /// The owning epic's state directory.
  pub state_dir: String,
  /// The host an agent runs on.
  pub host: Option<String>,
  /// A per-request host cap.
  pub host_cap: Option<i64>,
  /// A per-epic agent cap.
  pub epic_cap: Option<i64>,
  /// The worktree the work runs in.
  pub worktree: Option<String>,
}

/// The fields `patchLease` may change.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LeasePatch {
  /// The process group the lease owns.
  pub group_pid: Option<u32>,
  /// The lease's stage.
  pub stage: Option<String>,
  /// The last heartbeat, as `toISOString` prints it.
  pub heartbeat_at: Option<String>,
}

fn text<'a>(lease: &'a Ordered, key: &str) -> Option<&'a str> {
  match lease.get(key) {
    Some(Ordered::String(value)) => Some(value),
    _ => None,
  }
}

fn pid(lease: &Ordered, key: &str) -> Option<f64> {
  match lease.get(key) {
    Some(Ordered::Number(n)) => n.as_f64(),
    _ => None,
  }
}

/// A job lease is kept while its owner or its recorded group is alive.
fn keep(lease: &Ordered) -> Result<bool, String> {
  if text(lease, "type") != Some("job") || process_alive(pid(lease, "ownerPid").unwrap_or(0.0))? {
    return Ok(true);
  }
  let group = pid(lease, "groupPid")
    .and_then(whole)
    .and_then(|g| u32::try_from(g).ok());
  Ok(group.is_some_and(group::alive))
}

/// `reconcileJobs`: drops the job leases whose owner and group are gone; the count dropped.
///
/// # Errors
/// A process probe failure other than "no such process".
pub(crate) fn reconcile_jobs(state: &mut Ordered) -> Result<usize, String> {
  let Some(items) = leases_mut(state) else {
    return Ok(0);
  };
  let before = items.len();
  let mut kept = Vec::with_capacity(before);
  for lease in items.drain(..) {
    if keep(&lease)? {
      kept.push(lease);
    }
  }
  *items = kept;
  Ok(before - items.len())
}

/// `reconcileResourceJobs(root)`.
///
/// # Errors
/// A lock, state or probe failure.
pub fn reconcile_resource_jobs(root: &Path) -> Result<usize, String> {
  update_resources(root, reconcile_jobs)
}

/// `freshPressure(state)`: the stored pressure, re-sampled when 30 seconds old.
///
/// # Errors
/// An invalid or backwards sample.
pub(crate) fn fresh_pressure(state: &mut Ordered, now: SystemTime) -> Result<Pressure, String> {
  let now_ms = epoch_ms(now);
  let stored = state.get("pressure").and_then(pressure_of);
  if let Some(pressure) = stored
    .as_ref()
    .filter(|p| now_ms - p.sample.at < PRESSURE_SAMPLE_MS)
  {
    return Ok(pressure.clone());
  }
  let sample = sample_resources(stored.as_ref().map(|p| &p.sample), now_ms);
  let next = advance_pressure(stored.as_ref(), &sample)?;
  state.set("pressure", next.to_ordered());
  Ok(next)
}

/// `Date.now()` as a number; epoch milliseconds stay exact in an f64 until 2^53.
pub fn epoch_ms(now: SystemTime) -> f64 {
  epoch_millis(now).to_string().parse().unwrap_or(0.0)
}

/// `admitPressure(state, policy)`.
///
/// # Errors
/// `resource hold: <reason>` during a hold, or a sampling failure.
pub(crate) fn admit_pressure(
  state: &mut Ordered,
  policy: &ResourcePolicy,
  now: SystemTime,
) -> Result<(), String> {
  if !policy.pressure {
    return Ok(());
  }
  let pressure = fresh_pressure(state, now)?;
  match pressure.reason.filter(|_| pressure.held) {
    Some(reason) => Err(format!("resource hold: {reason}")),
    None => Ok(()),
  }
}

fn host_limit(req: &LeaseRequest, policy: &ResourcePolicy, host: &str) -> Result<i64, String> {
  let configured = policy
    .hosts
    .iter()
    .find(|(name, _)| name == host)
    .map_or(policy.max_agents, |(_, cap)| *cap);
  let limit = req.host_cap.unwrap_or(policy.max_agents).min(configured);
  if limit <= 0 {
    return Err("invalid host capacity".to_owned());
  }
  Ok(limit)
}

fn cooling(state: &Ordered, host: &str, now: SystemTime) -> bool {
  let until = state
    .get("cooldowns")
    .and_then(|cooldowns| cooldowns.get(host))
    .and_then(|cooldown| pid(cooldown, "until"))
    .unwrap_or(0.0);
  until > epoch_ms(now)
}

fn count(n: usize) -> i64 {
  i64::try_from(n).unwrap_or(i64::MAX)
}

/// `requireJobAdmission`: a job in a worktree whose agent is past `running` is refused.
fn admit_worktree_job(state: &Ordered, req: &LeaseRequest) -> Result<(), String> {
  let (LeaseKind::Job, Some(worktree)) = (req.kind, &req.worktree) else {
    return Ok(());
  };
  let agent = leases(state)
    .iter()
    .find(|l| text(l, "type") == Some("agent") && text(l, "worktree") == Some(worktree));
  match agent
    .and_then(|a| text(a, "stage"))
    .filter(|s| *s != "starting" && *s != "running")
  {
    Some(blocking) => Err(format!(
      "worktree job admission blocked by agent stage {blocking}"
    )),
    None => Ok(()),
  }
}

/// An agent's epic and host caps, and its host's cooldown.
fn admit_agent(
  state: &Ordered,
  policy: &ResourcePolicy,
  req: &LeaseRequest,
  live: &[&Ordered],
  now: SystemTime,
) -> Result<(), String> {
  let in_epic = live
    .iter()
    .filter(|l| text(l, "stateDir") == Some(&req.state_dir))
    .count();
  if req.epic_cap.is_some_and(|cap| count(in_epic) >= cap) {
    return Err("epic capacity exhausted".to_owned());
  }
  let Some(host) = req.host.as_deref().filter(|host| !host.is_empty()) else {
    return Ok(());
  };
  let limit = host_limit(req, policy, host)?;
  if cooling(state, host, now) {
    return Err(format!("{host} is cooling down"));
  }
  let on_host = live
    .iter()
    .filter(|l| text(l, "host") == Some(host) || text(l, "pendingHost") == Some(host))
    .count();
  if count(on_host) >= limit {
    return Err(format!("{host} capacity exhausted"));
  }
  Ok(())
}

/// The admission checks after reconciliation and pressure, in TypeScript's order.
fn admit(
  state: &Ordered,
  policy: &ResourcePolicy,
  req: &LeaseRequest,
  now: SystemTime,
) -> Result<(), String> {
  let kind = req.kind.name();
  let live: Vec<&Ordered> = leases(state)
    .iter()
    .filter(|l| text(l, "type") == Some(kind))
    .collect();
  admit_worktree_job(state, req)?;
  if live
    .iter()
    .any(|l| text(l, "stateDir") == Some(&req.state_dir) && text(l, "key") == Some(&req.key))
  {
    return Err("existing ownership; reconcile before retry".to_owned());
  }
  let limit = match req.kind {
    LeaseKind::Agent => policy.max_agents,
    LeaseKind::Job => policy.max_jobs,
  };
  if count(live.len()) >= limit {
    return Err(format!("{kind} capacity exhausted ({limit})"));
  }
  match req.kind {
    LeaseKind::Agent => admit_agent(state, policy, req, &live, now),
    LeaseKind::Job => Ok(()),
  }
}

fn valid_caps(req: &LeaseRequest) -> Result<(), String> {
  if req.host_cap.is_some_and(|cap| cap <= 0) {
    return Err("invalid host capacity".to_owned());
  }
  if req.epic_cap.is_some_and(|cap| cap <= 0) {
    return Err("invalid epic capacity".to_owned());
  }
  Ok(())
}

/// `acquireLease(root, req)`: a new lease, or the reason it is refused.
///
/// # Errors
/// An invalid cap, a pressure hold, existing ownership, exhausted capacity, a
/// cooling host, or a lock or state failure.
pub fn acquire_lease(root: &Path, req: &LeaseRequest, now: SystemTime) -> Result<Ordered, String> {
  valid_caps(req)?;
  update_resources(root, |state| {
    let policy = resource_policy(root)?;
    reconcile_jobs(state)?;
    admit_pressure(state, &policy, now)?;
    admit(state, &policy, req, now)?;
    let stamp = iso_millis(now);
    let mut lease = Ordered::Object(vec![
      ("token".to_owned(), string(&token())),
      ("type".to_owned(), string(req.kind.name())),
      ("key".to_owned(), string(&req.key)),
      ("stateDir".to_owned(), string(&req.state_dir)),
      ("ownerPid".to_owned(), number(f64::from(std::process::id()))),
      ("stage".to_owned(), string("starting")),
      ("createdAt".to_owned(), string(&stamp)),
      ("heartbeatAt".to_owned(), string(&stamp)),
    ]);
    if let Some(host) = &req.host {
      lease.set("host", string(host));
    }
    if let Some(worktree) = &req.worktree {
      lease.set("worktree", string(worktree));
    }
    leases_mut(state)
      .ok_or("invalid resource state")?
      .push(lease.clone());
    Ok(lease)
  })
}

/// `patchLease(root, token, patch)`.
///
/// # Errors
/// `resource lease lost` when no lease has `token`, or a lock or state failure.
pub fn patch_lease(root: &Path, token: &str, patch: &LeasePatch) -> Result<(), String> {
  if patch.group_pid == Some(0) {
    return Err("invalid resource lease patch".to_owned());
  }
  update_resources(root, |state| {
    let lease = leases_mut(state)
      .and_then(|items| items.iter_mut().find(|l| text(l, "token") == Some(token)))
      .ok_or("resource lease lost")?;
    if let Some(group) = patch.group_pid {
      lease.set("groupPid", number(f64::from(group)));
    }
    if let Some(stage) = &patch.stage {
      lease.set("stage", string(stage));
    }
    if let Some(at) = &patch.heartbeat_at {
      lease.set("heartbeatAt", string(at));
    }
    Ok(())
  })
}

/// `releaseLease(root, token)`: an agent's lease stays while a job of its worktree runs.
///
/// # Errors
/// `active jobs prevent agent lease release`, or a lock, state or probe failure.
pub fn release_lease(root: &Path, token: &str) -> Result<(), String> {
  update_resources(root, |state| {
    reconcile_jobs(state)?;
    let all = leases(state);
    let lease = all.iter().find(|l| text(l, "token") == Some(token));
    if let Some(worktree) = lease
      .filter(|l| text(l, "type") == Some("agent"))
      .and_then(|l| text(l, "worktree"))
      && all
        .iter()
        .any(|l| text(l, "type") == Some("job") && text(l, "worktree") == Some(worktree))
    {
      return Err("active jobs prevent agent lease release".to_owned());
    }
    if let Some(items) = leases_mut(state) {
      items.retain(|l| text(l, "token") != Some(token));
    }
    Ok(())
  })
}

#[cfg(test)]
#[path = "tests/leases_test.rs"]
mod tests;
