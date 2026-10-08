//! Hold launch and merge effects when GitHub primary capacity is scarce.

use crate::model::World;

const REST_FLOOR: u64 = 1_000;
const GRAPHQL_FLOOR: u64 = 500;

/// A low observed primary budget pauses effects; checks continue on schedule.
pub(crate) fn low(world: &World) -> bool {
  let rest = world
    .rest_rate
    .as_ref()
    .is_some_and(|rate| scarce(rate.remaining, rate.reset, REST_FLOOR, world.now_ms));
  let graphql = scarce(
    world.graphql_remaining,
    world.graphql_reset_at,
    GRAPHQL_FLOOR,
    world.now_ms,
  );
  rest || graphql
}

fn scarce(remaining: Option<u64>, reset_at: Option<u64>, floor: u64, now_ms: u64) -> bool {
  remaining.is_some_and(|value| value < floor)
    && reset_at.is_none_or(|reset| now_ms / 1_000 < reset)
}

#[cfg(test)]
#[path = "tests/github_budget_test.rs"]
mod tests;
