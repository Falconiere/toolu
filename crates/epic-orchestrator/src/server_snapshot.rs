//! Persist the resident engine's deadlines and GitHub budget observations.

use serde_json::json;

use crate::disk::write_value;
use crate::server::Engine;

impl Engine {
  /// Replace `watch.json` with the current fixed deadlines and budgets.
  ///
  /// # Errors
  /// The watch file cannot be replaced.
  pub(crate) fn persist_watch(&self) -> Result<(), String> {
    write_value(
      &self.paths.watch(),
      &json!({
        "version": 1,
        "nextCheckpointAt": self.world.checkpoint_at_ms,
        "checkpointQueue": [],
        "nextBudgetAt": self.world.now_ms,
        "budgetAlertReset": 0,
        "budgetHoldUntil": 0,
        "herdrFailures": self.world.herdr_failures,
        "herdrRetryAt": self.world.herdr_retry_at_ms,
        "github": self.world.watches,
        "githubHoldUntil": self.world.github_hold_until_ms,
        "githubRestRate": self.world.rest_rate,
        "githubGraphqlRemaining": self.world.graphql_remaining,
        "githubGraphqlResetAt": self.world.graphql_reset_at,
        "githubRestPoints": self.world.rest_points,
        "githubGraphqlPoints": self.world.graphql_points,
      }),
    )
  }
}

#[cfg(test)]
#[path = "tests/server_snapshot_test.rs"]
mod tests;
