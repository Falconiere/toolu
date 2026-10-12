//! Cleanup ownership (`finish-state.ts`): the recorded lease, or a legacy
//! bound worktree adopted durably; the job fence; and the release once
//! cleanup is proven.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde_json::json;
use toolu_engine::resources::binding::resource_binding;
use toolu_engine::resources::leases::{
  LeaseKind, LeaseRequest, acquire_lease, reconcile_resource_jobs,
};
use toolu_engine::resources::store::{leases, read_resource_state};
use toolu_engine::resources::transitions::{
  FenceError, active_jobs, fence_worktree, release_lease,
};
use toolu_runtime::json::ordered::Ordered;

use crate::launch::record::{Record, text, write_record};

/// The lease a cleanup holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Cleanup {
  /// The resource root.
  pub root: PathBuf,
  /// The agent lease token.
  pub token: String,
}

fn field(lease: &Ordered, key: &str) -> Option<String> {
  match lease.get(key) {
    Some(Ordered::String(value)) => Some(value.clone()),
    _ => None,
  }
}

/// Whether `lease` is this issue's agent lease on its worktree.
fn ours(lease: &Ordered, key: &str, state_dir: &str, worktree: &str) -> bool {
  field(lease, "type").as_deref() == Some("agent")
    && field(lease, "key").as_deref() == Some(key)
    && field(lease, "stateDir").as_deref() == Some(state_dir)
    && field(lease, "worktree").is_none_or(|bound| bound == worktree)
}

/// Adopt a legacy binding's lease (or a new one) into the record.
fn adopt(
  path: &Path,
  root: &Path,
  key: &str,
  state_dir: &str,
  record: &mut Record,
) -> Result<String, String> {
  let worktree = text(record, "worktree").unwrap_or("").to_owned();
  let state = read_resource_state(root)?;
  let matches: Vec<&Ordered> = leases(&state)
    .iter()
    .filter(|lease| ours(lease, key, state_dir, &worktree))
    .collect();
  if matches.len() > 1 {
    return Err("ambiguous legacy agent ownership".to_owned());
  }
  let token = if let Some(lease) = matches.first() {
    field(lease, "token").unwrap_or_default()
  } else {
    let request = LeaseRequest {
      kind: LeaseKind::Agent,
      key: key.to_owned(),
      state_dir: state_dir.to_owned(),
      host: None,
      host_cap: None,
      epic_cap: None,
      worktree: Some(worktree),
    };
    field(&acquire_lease(root, &request, SystemTime::now())?, "token").unwrap_or_default()
  };
  record.insert(
    "resource_root".to_owned(),
    json!(root.display().to_string()),
  );
  record.insert("lease_token".to_owned(), json!(token));
  write_record(path, record)?;
  Ok(token)
}

/// Resolve the recorded lease, or adopt a bound legacy worktree, then fence
/// the worktree's jobs. `None` when nothing owns it.
///
/// # Errors
/// The binding or the lease belongs to another issue, or the store fails.
pub(crate) fn fence_cleanup_ownership(
  path: &Path,
  state_dir: &str,
  key: &str,
  record: &mut Record,
) -> Result<Option<Cleanup>, String> {
  let worktree = text(record, "worktree").unwrap_or("").to_owned();
  let binding = if Path::new(&worktree).is_dir() {
    resource_binding(Path::new(&worktree))?
  } else {
    None
  };
  let mut root = text(record, "resource_root").map(PathBuf::from);
  let mut token = text(record, "lease_token").map(str::to_owned);
  if let Some(binding) = &binding {
    if binding.key != key || binding.state_dir != state_dir {
      return Err("worktree resource binding does not match issue ownership".to_owned());
    }
    if root.as_ref().is_some_and(|root| *root != binding.root) {
      return Err("recorded resource root differs from worktree binding".to_owned());
    }
    if root.is_none() {
      token = Some(adopt(path, &binding.root, key, state_dir, record)?);
      root = Some(binding.root.clone());
    }
  }
  let (Some(root), Some(token)) = (root, token) else {
    return Ok(None);
  };
  let state = read_resource_state(&root)?;
  let recorded = leases(&state)
    .iter()
    .find(|lease| field(lease, "token").as_deref() == Some(token.as_str()));
  if !recorded.is_some_and(|lease| ours(lease, key, state_dir, &worktree)) {
    return Err("recorded agent resource ownership does not match issue".to_owned());
  }
  match fence_worktree(&root, &token, &worktree, SystemTime::now()) {
    Ok(_) | Err(FenceError::JobsActive) => Ok(Some(Cleanup { root, token })),
    Err(FenceError::Failed(message)) => Err(message),
  }
}

/// Release the recorded lease once cleanup is proven; a lease already gone
/// is fine.
///
/// # Errors
/// The lease belongs to another issue, or the store fails.
pub(crate) fn release_recorded_ownership(record: &Record, state_dir: &str) -> Result<(), String> {
  let (Some(root), Some(token)) = (text(record, "resource_root"), text(record, "lease_token"))
  else {
    return Ok(());
  };
  let state = read_resource_state(Path::new(root))?;
  let Some(lease) = leases(&state)
    .iter()
    .find(|lease| field(lease, "token").as_deref() == Some(token))
  else {
    return Ok(());
  };
  let (key, worktree) = (
    text(record, "key").unwrap_or(""),
    text(record, "worktree").unwrap_or(""),
  );
  if !ours(lease, key, state_dir, worktree) {
    return Err("recorded agent resource ownership does not match issue".to_owned());
  }
  release_lease(Path::new(root), token)
}

/// Refuse while a managed job still runs in the worktree.
///
/// # Errors
/// A job remains, or the store fails.
pub(crate) fn verify_no_jobs(ownership: Option<&Cleanup>, worktree: &str) -> Result<(), String> {
  let Some(ownership) = ownership else {
    return Ok(());
  };
  reconcile_resource_jobs(&ownership.root)?;
  if active_jobs(&ownership.root, worktree)? {
    return Err("managed job ownership remains; wait or reconcile before cleanup".to_owned());
  }
  Ok(())
}

#[cfg(test)]
#[path = "tests/ownership_test.rs"]
mod tests;
