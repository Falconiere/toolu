//! The standalone MCP hook's narrow entry into the existing blocker gate.

use toolu_protocol::decision::Decision;
use toolu_protocol::normalized::NormalizedEvent;
use toolu_runtime::registry::rule::RuleContext;

use crate::gates::mcp_blocker::evaluate;

/// Check one MCP call without walking the other built-ins or registry modules.
///
/// # Errors
/// Returns the blocker gate's configuration or settings error.
pub fn check(
  event: &NormalizedEvent,
  ctx: &RuleContext<'_>,
) -> (Result<Decision, String>, Vec<String>) {
  let mut warnings = Vec::new();
  let result = evaluate(event, ctx, &mut warnings);
  (result, warnings)
}

#[cfg(test)]
#[path = "tests/mcp_hook_test.rs"]
mod tests;
