//! Durable launch ownership, reserved before any external mutation
//! (`admission.ts`): the agent lease, its replacement on another host, the
//! per-issue launch lock, and each attempt's persisted stage.

mod hc;

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use toolu_engine::resources::agents::reconcile_agent_leases;
use toolu_engine::resources::leases::{
  LeaseKind, LeasePatch, LeaseRequest, acquire_lease, patch_lease, reconcile_resource_jobs,
};
use toolu_engine::resources::lock::acquire_lock;
use toolu_engine::resources::store::{leases, read_resource_state, resource_home};
use toolu_engine::resources::transitions::{
  FenceError, MigrationCaps, active_jobs, fence_worktree, migrate_agent_lease,
  prepare_agent_migration,
};
use toolu_runtime::env::Env;
use toolu_runtime::json::ordered::Ordered;

use super::failure::Failure;
use super::lifecycle::{Expected, Herdr, Outcome, shutdown_agent};
use super::record::{Record, text, write_record};
use crate::herdr::Call;
use crate::hosts::{HostKind, host_kind};
use crate::route::load_host_pool;

/// The lease a launch attempt holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Owner {
  /// The resource root.
  pub root: PathBuf,
  /// The lease token.
  pub token: String,
}

/// What a reservation is for.
pub(crate) struct Reserve<'a> {
  /// The epic's state directory.
  pub state: &'a Path,
  /// The issue key.
  pub key: &'a str,
  /// The host launching.
  pub host: HostKind,
  /// The epic's agent cap.
  pub max_parallel: u64,
  /// Move a live agent to `host`.
  pub replace: bool,
  /// The environment.
  pub env: &'a Env,
}

fn field(lease: &Ordered, key: &str) -> Option<String> {
  match lease.get(key) {
    Some(Ordered::String(value)) => Some(value.clone()),
    _ => None,
  }
}

fn cap(value: u64) -> i64 {
  i64::try_from(value).unwrap_or(i64::MAX)
}

/// Persist `record` at `stage` with its ownership, and mirror the stage, the
/// worktree, the pane and the session onto the lease.
///
/// # Errors
/// The record or the lease cannot be written.
pub(crate) fn persist_attempt(
  dir: &Path,
  key: &str,
  owner: &Owner,
  record: &mut Record,
  stage: &str,
) -> Result<(), Failure> {
  let resource_root = owner.root.display().to_string();
  record.insert("resource_root".to_owned(), resource_root.into());
  record.insert("lease_token".to_owned(), owner.token.clone().into());
  record.insert("attempt_id".to_owned(), owner.token.clone().into());
  record.insert("stage".to_owned(), stage.into());
  write_record(&dir.join("issues").join(format!("{key}.json")), record)?;
  let patch = LeasePatch {
    stage: Some(stage.to_owned()),
    worktree: text(record, "worktree").map(str::to_owned),
    pane: text(record, "pane_id").map(str::to_owned),
    session: text(record, "session_id").map(str::to_owned),
    ..LeasePatch::default()
  };
  patch_lease(&owner.root, &owner.token, &patch).map_err(Failure::unknown)
}

/// The saved host lease's token and host for this issue, if any.
fn saved_lease(
  root: &Path,
  input: &Reserve<'_>,
) -> Result<Option<(String, Option<String>)>, Failure> {
  let state_dir = input.state.display().to_string();
  let resources = read_resource_state(root).map_err(Failure::unknown)?;
  Ok(leases(&resources).iter().find_map(|lease| {
    let mine = field(lease, "type").as_deref() == Some("agent")
      && field(lease, "stateDir").as_deref() == Some(state_dir.as_str())
      && field(lease, "key").as_deref() == Some(input.key);
    mine.then(|| {
      (
        field(lease, "token").unwrap_or_default(),
        field(lease, "host"),
      )
    })
  }))
}

/// The cap of `input.host` in the saved pool (the epic cap without one).
fn selected_cap(input: &Reserve<'_>) -> Result<u64, Failure> {
  let fallback = vec![(input.host, input.max_parallel)];
  let pool = load_host_pool(&input.state.join("pool.json"), fallback).map_err(Failure::unknown)?;
  pool
    .iter()
    .find(|(kind, _)| *kind == input.host)
    .map(|(_, cap)| *cap)
    .ok_or_else(|| {
      Failure::unknown(format!(
        "{} is no longer in the host pool; reroute first",
        input.host.name()
      ))
    })
}

/// A fresh agent lease, refused at capacity.
fn acquire(root: PathBuf, input: &Reserve<'_>, selected: u64) -> Result<Owner, Failure> {
  let request = LeaseRequest {
    kind: LeaseKind::Agent,
    key: input.key.to_owned(),
    state_dir: input.state.display().to_string(),
    host: Some(input.host.name().to_owned()),
    host_cap: Some(cap(selected)),
    epic_cap: Some(cap(input.max_parallel)),
    worktree: None,
  };
  let lease = acquire_lease(&root, &request, SystemTime::now()).map_err(Failure::unknown)?;
  let token = field(&lease, "token").unwrap_or_default();
  Ok(Owner { root, token })
}

/// Reserve the agent lease of `input.key` on `input.host`, or move a live
/// agent there when `input.replace` (destination reserved before the source
/// stops). Refuses before any external work at capacity.
///
/// # Errors
/// A malformed pool, a foreign or unverifiable lease, capacity, or a
/// replacement whose old workload remains.
pub(crate) fn launch_reservation(
  input: &Reserve<'_>,
  record: &mut Record,
  herdr: &mut Herdr<'_>,
) -> Result<Owner, Failure> {
  let root = text(record, "resource_root").map_or_else(|| resource_home(input.env), PathBuf::from);
  let selected = selected_cap(input)?;
  reclaim_dead_agents(&root, herdr);
  let Some((token, saved_host)) = saved_lease(&root, input)? else {
    return acquire(root, input, selected);
  };
  if text(record, "lease_token") != Some(token.as_str()) {
    return Err(Failure::unknown(
      "existing launch ownership differs; finish/reconcile it before retry",
    ));
  }
  let owner = Owner { root, token };
  if saved_host.as_deref() == Some(input.host.name()) && !input.replace {
    return Ok(owner);
  }
  let source = Source::of(record, saved_host.as_deref(), input.replace, selected)?;
  prepare_agent_migration(
    &owner.root,
    &owner.token,
    input.host.name(),
    source.caps(input),
    SystemTime::now(),
  )
  .map_err(Failure::unknown)?;
  match migrate(input, record, herdr, &owner, &source) {
    Ok(()) => Ok(owner),
    Err(err) => {
      record.extend(err.details());
      persist_attempt(input.state, input.key, &owner, record, "uncertain")?;
      Err(err)
    }
  }
}

/// Free the agent leases whose owner exited and whose agent herdr no longer
/// lists (#583). herdr is asked only for such a lease, and only once; without
/// its answer nothing is proven gone, so nothing is freed.
fn reclaim_dead_agents(root: &Path, herdr: &mut Herdr<'_>) {
  let mut listing: Option<Option<(toolu_mux::Backend, Vec<serde_json::Value>)>> = None;
  let listed = |name: &str, pane: Option<&str>| {
    let agents = listing.get_or_insert_with(|| {
      let reply = herdr.call(&Call::AgentList).ok()?;
      let backend = herdr.client.backend()?;
      Some((backend, reply.get("agents")?.as_array()?.clone()))
    });
    agents.as_ref().is_none_or(|(backend, agents)| {
      if !pane_matches_backend(pane, *backend) {
        return true;
      }
      agents.iter().any(|agent| {
        let text = |key: &str| agent.get(key).and_then(serde_json::Value::as_str);
        text("name") == Some(name) || pane.is_some_and(|p| text("pane_id") == Some(p))
      })
    })
  };
  // A failed reclaim leaves the leases, as before; admission reports capacity.
  let _freed = reconcile_agent_leases(root, listed);
}

fn pane_matches_backend(pane: Option<&str>, backend: toolu_mux::Backend) -> bool {
  let Some(pane) = pane else {
    return false;
  };
  match backend {
    toolu_mux::Backend::HerdrApp => pane.starts_with("hc:"),
    toolu_mux::Backend::Tmux => pane.starts_with('%'),
    toolu_mux::Backend::Herdr => !pane.starts_with("hc:") && !pane.starts_with('%'),
  }
}

/// The live agent a replacement moves away from.
struct Source {
  from: HostKind,
  pane: String,
  worktree: String,
  cap: u64,
}

impl Source {
  /// The verified source of a replacement, or a refusal.
  fn of(record: &Record, host: Option<&str>, replace: bool, cap: u64) -> Result<Self, Failure> {
    let from = host.map(host_kind).transpose().map_err(Failure::unknown)?;
    let (Some(from), Some(pane), Some(worktree), true) = (
      from,
      text(record, "pane_id"),
      text(record, "worktree"),
      replace,
    ) else {
      return Err(Failure::unknown(
        "replacement requires verified ownership and explicit --replace",
      ));
    };
    Ok(Self {
      from,
      pane: pane.to_owned(),
      worktree: worktree.to_owned(),
      cap,
    })
  }

  fn caps(&self, input: &Reserve<'_>) -> MigrationCaps {
    MigrationCaps {
      host_cap: cap(self.cap),
      epic_cap: Some(cap(input.max_parallel)),
    }
  }
}

/// Fence the worktree, stop the source agent, verify its workload is gone,
/// then move the lease.
fn migrate(
  input: &Reserve<'_>,
  record: &mut Record,
  herdr: &mut Herdr<'_>,
  owner: &Owner,
  source: &Source,
) -> Result<(), Failure> {
  persist_attempt(input.state, input.key, owner, record, "replacing")?;
  match fence_worktree(
    &owner.root,
    &owner.token,
    &source.worktree,
    SystemTime::now(),
  ) {
    // Explicit replacement cancels existing jobs; the persisted fence blocks new ones.
    Ok(_) | Err(FenceError::JobsActive) => {}
    Err(FenceError::Failed(message)) => return Err(Failure::unknown(message)),
  }
  let expected = Expected {
    kind: source.from,
    pane: &source.pane,
    cwd: &source.worktree,
  };
  let stopped = if text(record, "backend") == Some("herdr-app") {
    hc::stop(record, herdr)?
  } else {
    shutdown_agent(herdr, input.key, &expected)?
  };
  reconcile_resource_jobs(&owner.root).map_err(Failure::unknown)?;
  let busy = active_jobs(&owner.root, &source.worktree).map_err(Failure::unknown)?;
  if stopped != Outcome::Exited || busy {
    return Err(Failure::unknown(
      "old host workload remains; replacement refused",
    ));
  }
  persist_attempt(input.state, input.key, owner, record, "replacing")?;
  migrate_agent_lease(
    &owner.root,
    &owner.token,
    input.host.name(),
    SystemTime::now(),
  )
  .map(|_| ())
  .map_err(Failure::unknown)
}

/// Run `work` holding `launch-locks/<key>`, shared by every verb and the engine.
///
/// # Errors
/// Another launch holds the lock, or `work` fails.
pub(crate) fn with_launch_ownership<T>(
  state: &Path,
  key: &str,
  work: impl FnOnce() -> Result<T, Failure>,
) -> Result<T, Failure> {
  let path = state.join("launch-locks").join(key);
  let lock = acquire_lock(&path, Duration::ZERO)
    .map_err(Failure::unknown)?
    .ok_or_else(|| Failure::unknown(format!("launch already in progress for {key}")))?;
  let result = work();
  lock.release();
  result
}

#[cfg(test)]
#[path = "tests/admission_test.rs"]
mod tests;
