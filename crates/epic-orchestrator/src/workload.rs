//! The workload processes one worktree owns (`scripts/workload.ts`): every
//! process whose cwd is inside it, and their descendants wherever they moved,
//! excluding the caller and the agent's shell. Finish and a replacing launch
//! refuse while any of them lives.

use std::collections::BTreeSet;
use std::path::Path;
use std::time::{Duration, Instant};

mod linux;
mod mac;

/// One owned process and its birth identity, so a reused PID is not mistaken
/// for it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct OwnedWorkload {
  /// Process id.
  pub pid: u32,
  /// Process group id.
  pub group: u32,
  /// `linux:<starttime>` or `mac:<lstart>`.
  pub started: String,
}

/// One process of the inventory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProcessRecord {
  /// Process id.
  pub pid: u32,
  /// Parent process id.
  pub parent: u32,
  /// Process group id.
  pub group: u32,
  /// Birth identity.
  pub started: String,
  /// The cwd, when readable.
  pub cwd: Option<String>,
  /// The cwd exists but cannot be read (another user's, or not dumpable).
  pub cwd_unavailable: bool,
}

fn processes() -> Result<Vec<ProcessRecord>, String> {
  match std::env::consts::OS {
    "linux" => linux::processes(),
    "macos" => mac::processes(),
    other => Err(format!("process inventory is unavailable on {other}")),
  }
}

fn inside(cwd: Option<&str>, root: &str) -> bool {
  cwd.is_some_and(|cwd| {
    cwd == root
      || cwd
        .strip_prefix(root)
        .is_some_and(|rest| rest.starts_with('/'))
  })
}

/// The workload `worktree` owns, by PID, excluding this process and `shell`.
///
/// # Errors
/// The worktree cannot be resolved, the inventory fails, or a process in an
/// owned group has an unreadable cwd, so ownership is uncertain.
pub(crate) fn owned_workload(
  worktree: &Path,
  shell: Option<u32>,
) -> Result<Vec<OwnedWorkload>, String> {
  let root = std::fs::canonicalize(worktree)
    .map_err(|err| format!("cannot resolve worktree {}: {err}", worktree.display()))?;
  let excluded: BTreeSet<u32> = [Some(std::process::id()), shell]
    .into_iter()
    .flatten()
    .collect();
  select_owned(&processes()?, &root.display().to_string(), &excluded)
}

/// The owned subset of `inventory`: cwd inside `root`, then descendants.
///
/// # Errors
/// A process outside the owned set shares an owned group and its cwd is
/// unreadable.
pub(crate) fn select_owned(
  inventory: &[ProcessRecord],
  root: &str,
  excluded: &BTreeSet<u32>,
) -> Result<Vec<OwnedWorkload>, String> {
  let mut owned: BTreeSet<u32> = inventory
    .iter()
    .filter(|record| !excluded.contains(&record.pid) && inside(record.cwd.as_deref(), root))
    .map(|record| record.pid)
    .collect();
  loop {
    let more: Vec<u32> = inventory
      .iter()
      .filter(|record| {
        !excluded.contains(&record.pid)
          && owned.contains(&record.parent)
          && !owned.contains(&record.pid)
      })
      .map(|record| record.pid)
      .collect();
    if more.is_empty() {
      break;
    }
    owned.extend(more);
  }
  let groups: BTreeSet<u32> = inventory
    .iter()
    .filter(|record| owned.contains(&record.pid))
    .map(|record| record.group)
    .collect();
  if let Some(unknown) = inventory.iter().find(|record| {
    !owned.contains(&record.pid) && record.cwd_unavailable && groups.contains(&record.group)
  }) {
    return Err(format!(
      "cannot determine worktree ownership of process {}",
      unknown.pid
    ));
  }
  let mut out: Vec<OwnedWorkload> = inventory
    .iter()
    .filter(|record| owned.contains(&record.pid))
    .map(|record| OwnedWorkload {
      pid: record.pid,
      group: record.group,
      started: record.started.clone(),
    })
    .collect();
  out.sort();
  Ok(out)
}

/// Whether every recorded process is gone or its PID belongs to a later process.
///
/// # Errors
/// The inventory fails.
pub(crate) fn verify_owned_exit(records: &[OwnedWorkload]) -> Result<bool, String> {
  if records.is_empty() {
    return Ok(true);
  }
  match std::env::consts::OS {
    "linux" => {
      for record in records {
        if linux::record(record.pid)?.is_some_and(|now| now.started == record.started) {
          return Ok(false);
        }
      }
      Ok(true)
    }
    "macos" => {
      let current = mac::identities()?;
      Ok(records.iter().all(|record| {
        !current
          .iter()
          .any(|now| now.pid == record.pid && now.started == record.started)
      }))
    }
    other => Err(format!("process inventory is unavailable on {other}")),
  }
}

/// Poll up to `grace` until every recorded process is gone and the worktree
/// owns no new one: a departing agent's children take a moment to exit.
///
/// # Errors
/// The inventory fails, or ownership is uncertain.
pub(crate) fn await_owned_exit(
  worktree: &Path,
  shell: Option<u32>,
  owned: &[OwnedWorkload],
  grace: Duration,
) -> Result<bool, String> {
  let deadline = Instant::now() + grace;
  loop {
    if verify_owned_exit(owned)? && owned_workload(worktree, shell)?.is_empty() {
      return Ok(true);
    }
    if Instant::now() >= deadline {
      return Ok(false);
    }
    std::thread::sleep(Duration::from_millis(500));
  }
}

#[cfg(test)]
#[path = "tests/workload_test.rs"]
mod tests;
