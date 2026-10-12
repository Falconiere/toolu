//! Hold launch and merge effects when GitHub primary capacity is scarce:
//! under `EPIC_GH_CORE_FLOOR` (default 1000) or `EPIC_GH_GRAPHQL_FLOOR`
//! (default 500), the single owner of the floors.

use crate::model::World;

/// A low primary budget or active retry hold pauses launch and merge effects.
pub(crate) fn effects_held(world: &World) -> bool {
  let rest = world
    .rest_rate
    .as_ref()
    .is_some_and(|rate| scarce(rate.remaining, rate.reset, world.floors.0, world.now_ms));
  let graphql = scarce(
    world.graphql_remaining,
    world.graphql_reset_at,
    world.floors.1,
    world.now_ms,
  );
  rest || graphql || world.now_ms < world.github_hold_until_ms
}

fn scarce(remaining: Option<u64>, reset_at: Option<u64>, floor: u64, now_ms: u64) -> bool {
  remaining.is_some_and(|value| value < floor)
    && reset_at.is_none_or(|reset| now_ms / 1_000 < reset)
}

#[cfg(test)]
#[path = "tests/github_budget_test.rs"]
mod tests;
