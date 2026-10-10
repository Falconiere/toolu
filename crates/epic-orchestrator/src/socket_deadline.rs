//! The resident socket's next wake time, including GitHub's global hold.

use std::time::{Duration, Instant};

use crate::model::World;
use crate::schedule::next_stall_ms;
use crate::socket::Waiter;

/// Whether a local stall deadline has elapsed.
pub(crate) fn stall_ready(world: &World) -> bool {
  next_stall_ms(world).is_some_and(|at| at <= world.now_ms)
}

/// Whether a GitHub watch may send a request now.
pub(crate) fn github_ready(world: &World) -> bool {
  world.github_hold_until_ms <= world.now_ms
    && world
      .watches
      .values()
      .any(|watch| watch.due_at_ms(world.now_ms) <= world.now_ms)
}

/// Earliest local or GitHub deadline for the socket receive timeout.
pub(crate) fn wake_after(world: &World, waiters: &[Waiter], next_tick: Instant) -> Duration {
  let until_tick = next_tick.saturating_duration_since(Instant::now());
  let until_wait = waiters
    .iter()
    .map(|waiter| waiter.deadline)
    .min()
    .map(|deadline| deadline.saturating_duration_since(Instant::now()));
  let until_stall =
    next_stall_ms(world).map(|at| Duration::from_millis(at.saturating_sub(world.now_ms)));
  let wake = until_wait.map_or(until_tick, |wait| until_tick.min(wait));
  let wake = until_stall.map_or(wake, |stall| wake.min(stall));
  let until_github = world
    .watches
    .values()
    .map(|watch| {
      watch
        .due_at_ms(world.now_ms)
        .max(world.github_hold_until_ms)
    })
    .min()
    .map(|at| Duration::from_millis(at.saturating_sub(world.now_ms)));
  until_github.map_or(wake, |github| wake.min(github))
}

#[cfg(test)]
#[path = "tests/socket_deadline_test.rs"]
mod tests;
