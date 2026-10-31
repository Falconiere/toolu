//! When a scheduled check may skip pr-babysit's collection (#761). A pull
//! request watch remembers its last collection that settled at keep-going in
//! babysit, with nothing GitHub was still computing. A later check reuses that
//! outcome only when it is scheduled, every REST probe answered `304` and no
//! array ended on a full page, the issue is still in the same babysit phase
//! (never ready), the babysit state file still holds the bytes that tick
//! wrote, and less than [`REFRESH_MS`] has passed. A skip decides nothing, so
//! it can never queue a merge.

use std::path::Path;
use std::time::Duration;

use toolu_engine::babysit::{BabysitTick, TickRequest, state_digest};

use crate::babysit::{self, Checked, Next};
use crate::model::World;
use crate::paths::Paths;
use crate::watch::{CHECK_MS, Kind};

/// The longest a keep-going collection is reused. REST `ETag`s miss
/// GraphQL-only changes, such as a thread resolved in the UI, so those are seen
/// at most this late.
pub(crate) const REFRESH_MS: u64 = 5 * CHECK_MS;

/// A pull request watch's last collection that settled at keep-going.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Collected {
  /// The engine time of the collection, in Unix milliseconds.
  pub at_ms: u64,
  /// The digest of the babysit state file bytes that collection wrote.
  pub state_digest: u64,
  /// The issue's `phase_at_ms` then; any phase change since collects.
  pub phase_at_ms: u64,
}

/// What one check did about babysit.
pub(crate) enum Babysit {
  /// The watch's issue is not waiting on babysit.
  Idle,
  /// The collection was skipped, reusing the one made at `at_ms`.
  Skipped {
    /// The engine time of the reused collection.
    at_ms: u64,
  },
  /// The tick ran.
  Checked(Checked),
}

impl Babysit {
  /// The tick's result, when it ran.
  pub(crate) fn checked(&self) -> Option<&Checked> {
    match self {
      Self::Checked(checked) => Some(checked),
      Self::Idle | Self::Skipped { .. } => None,
    }
  }
}

/// Run `watch_key`'s babysit tick unless its memo is reusable. `quiet` is a
/// scheduled check whose REST probe answered `304` everywhere.
pub(crate) fn run(
  world: &mut World,
  watch_key: &str,
  quiet: bool,
  tick: &dyn BabysitTick,
  paths: &Paths,
) -> Babysit {
  let Some((request, babysit_since)) = request(world, watch_key, paths) else {
    // A memo never outlives a stint outside babysit or ready.
    remember(world, watch_key, None);
    return Babysit::Idle;
  };
  if quiet && let Some(memo) = reusable(world, watch_key, babysit_since, &request.state_file) {
    return Babysit::Skipped { at_ms: memo.at_ms };
  }
  let checked = babysit::check(tick, &request);
  // Only a settled keep-going collection made in babysit is reused, compared
  // against the bytes that tick wrote rather than a later read of the file.
  let memo = match (babysit_since, checked.state_digest) {
    (Some(phase_at_ms), Some(state_digest))
      if checked.next == Next::KeepGoing && checked.settled =>
    {
      Some(Collected {
        at_ms: world.now_ms,
        state_digest,
        phase_at_ms,
      })
    }
    _ => None,
  };
  remember(world, watch_key, memo);
  Babysit::Checked(checked)
}

fn remember(world: &mut World, watch_key: &str, memo: Option<Collected>) {
  if let Some(watch) = world.watches.get_mut(watch_key) {
    watch.babysit = memo;
  }
}

/// The memo a quiet check of `watch_key` may reuse; `babysit_since` is the
/// issue's `phase_at_ms` while it is in babysit.
fn reusable(
  world: &World,
  watch_key: &str,
  babysit_since: Option<u64>,
  state_file: &Path,
) -> Option<Collected> {
  let watch = world.watches.get(watch_key)?;
  let memo = watch.babysit?;
  let recent = world
    .now_ms
    .checked_sub(memo.at_ms)
    .is_some_and(|age| age < REFRESH_MS);
  let same_phase = babysit_since == Some(memo.phase_at_ms);
  // The file is read only once the cheap checks pass.
  let unchanged =
    || std::fs::read(state_file).is_ok_and(|bytes| state_digest(&bytes) == memo.state_digest);
  (recent && same_phase && watch.full_tails.is_empty() && unchanged()).then_some(memo)
}

/// The babysit request of `watch_key`'s pull request while its issue waits in
/// babysit or ready, with the issue's `phase_at_ms` when it is in babysit.
fn request(world: &World, watch_key: &str, paths: &Paths) -> Option<(TickRequest, Option<u64>)> {
  let watch = world.watches.get(watch_key)?;
  let Kind::Pr { key, repo, number } = &watch.kind else {
    return None;
  };
  let issue = world.issues.get(key)?;
  if !matches!(issue.phase.as_str(), "babysit" | "ready")
    || matches!(issue.stage.as_str(), "cleaning" | "closed" | "merged")
  {
    return None;
  }
  let babysit_since = (issue.phase == "babysit").then_some(issue.phase_at_ms);
  let request = TickRequest {
    repo: repo.clone(),
    number: *number,
    state_file: paths
      .root
      .join("babysit")
      .join(format!("{}-{number}.json", repo.replace('/', "__"))),
    now: Some(toolu_state::time::iso_seconds(
      std::time::UNIX_EPOCH + Duration::from_millis(world.now_ms),
    )),
  };
  Some((request, babysit_since))
}

#[cfg(test)]
#[path = "tests/babysit_fresh_test.rs"]
mod tests;
