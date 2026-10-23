//! Reclaiming agent leases (#583): a lease whose owner process has exited and
//! whose agent is gone, because it never started or herdr no longer lists it,
//! would otherwise count against capacity forever.

use std::path::Path;

use toolu_runtime::json::ordered::Ordered;
use toolu_runtime::process::group;

use super::leases::{pid, text};
use super::lock::{process_alive, whole};
use super::store::{leases_mut, update_resources};

/// An agent lease is reclaimable once its owner and recorded group are dead
/// and `is_live` does not know its agent.
fn reclaimable(
  lease: &Ordered,
  is_live: &mut impl FnMut(&str, Option<&str>) -> bool,
) -> Result<bool, String> {
  if text(lease, "type") != Some("agent") || process_alive(pid(lease, "ownerPid").unwrap_or(0.0))? {
    return Ok(false);
  }
  let group_alive = pid(lease, "groupPid")
    .and_then(whole)
    .and_then(|g| u32::try_from(g).ok())
    .is_some_and(group::alive);
  let key = text(lease, "key").unwrap_or_default();
  Ok(!group_alive && !is_live(key, text(lease, "pane")))
}

/// Drops the agent leases whose owner is dead and whose agent `is_live`
/// (given its key and recorded pane) does not know; the count dropped.
///
/// # Errors
/// A lock, state or process probe failure.
pub fn reconcile_agent_leases(
  root: &Path,
  mut is_live: impl FnMut(&str, Option<&str>) -> bool,
) -> Result<usize, String> {
  update_resources(root, |state| {
    let Some(items) = leases_mut(state) else {
      return Ok(0);
    };
    let before = items.len();
    let mut kept = Vec::with_capacity(before);
    for lease in items.drain(..) {
      if !reclaimable(&lease, &mut is_live)? {
        kept.push(lease);
      }
    }
    *items = kept;
    Ok(before - items.len())
  })
}

#[cfg(test)]
#[path = "tests/agents_test.rs"]
mod tests;
