//! When a scheduled check may skip pr-babysit's collection (#761). A pull
//! request watch remembers its last collection that settled at keep-going.
//! A later check reuses that outcome only when it is scheduled, every REST
//! probe answered `304`, the issue is in babysit (never ready), the babysit
//! state file is byte-identical, and less than [`REFRESH_MS`] has passed.
//! A skip decides nothing, so it can never queue a merge.

use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::Path;
use std::time::Duration;

use toolu_engine::babysit::{BabysitTick, TickRequest};

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
  /// The hash of the babysit state file's bytes right after it.
  pub state_hash: u64,
}

/// Run `watch_key`'s babysit tick unless its memo is reusable. `quiet` is a
/// scheduled check whose REST probe was all `304` with no change. Returns the
/// tick's result, or the reused collection time of a skip; neither when the
/// issue is not waiting on babysit.
pub(crate) fn run(
  world: &mut World,
  watch_key: &str,
  quiet: bool,
  tick: &dyn BabysitTick,
  paths: &Paths,
) -> (Option<Checked>, Option<u64>) {
  let Some(request) = request(world, watch_key, paths) else {
    return (None, None);
  };
  if quiet && let Some(memo) = reusable(world, watch_key, &request.state_file) {
    return (None, Some(memo.at_ms));
  }
  let checked = babysit::check(tick, &request);
  // shortcut: a worker write landing between the tick and this read joins the
  // memo; REFRESH_MS bounds the delay, and the worker's report forces a check.
  let memo = (checked.next == Next::KeepGoing)
    .then(|| state_hash(&request.state_file))
    .flatten()
    .map(|state_hash| Collected {
      at_ms: world.now_ms,
      state_hash,
    });
  if let Some(watch) = world.watches.get_mut(watch_key) {
    watch.babysit = memo;
  }
  (Some(checked), None)
}

/// The memo a quiet check of `watch_key` may reuse.
fn reusable(world: &World, watch_key: &str, state_file: &Path) -> Option<Collected> {
  let memo = world.watches.get(watch_key)?.babysit?;
  let issue = crate::watch::issue_of(world, watch_key).and_then(|key| world.issues.get(key))?;
  let recent = world
    .now_ms
    .checked_sub(memo.at_ms)
    .is_some_and(|age| age < REFRESH_MS);
  let unchanged = state_hash(state_file) == Some(memo.state_hash);
  (recent && issue.phase == "babysit" && unchanged).then_some(memo)
}

/// The hash of `path`'s bytes; `None` when it cannot be read.
fn state_hash(path: &Path) -> Option<u64> {
  let bytes = std::fs::read(path).ok()?;
  let mut hasher = DefaultHasher::new();
  bytes.hash(&mut hasher);
  Some(hasher.finish())
}

/// The babysit request of `watch_key`'s pull request, while its issue waits
/// in babysit or ready.
fn request(world: &World, watch_key: &str, paths: &Paths) -> Option<TickRequest> {
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
  Some(TickRequest {
    repo: repo.clone(),
    number: *number,
    state_file: paths
      .root
      .join("babysit")
      .join(format!("{}-{number}.json", repo.replace('/', "__"))),
    now: Some(toolu_state::time::iso_seconds(
      std::time::UNIX_EPOCH + Duration::from_millis(world.now_ms),
    )),
  })
}

#[cfg(test)]
#[path = "tests/babysit_fresh_test.rs"]
mod tests;
