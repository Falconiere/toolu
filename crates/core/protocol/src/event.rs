//! Host events, split into those whose hook can prevent the action and the rest.

/// Events whose hook can prevent the action: a missing, crashed or incompatible
/// binary must block them. The same set as `ENFORCING_EVENTS` in
/// `packages/toolu-core/src/launcher/launcher.ts`.
pub const ENFORCING_EVENTS: &[&str] = &["PreToolUse", "PermissionRequest"];

/// Whether a hook registered under `event` enforces (blocks on failure).
pub fn is_enforcing(event: &str) -> bool {
  ENFORCING_EVENTS.contains(&event)
}

#[cfg(test)]
#[path = "tests/event_test.rs"]
mod tests;
