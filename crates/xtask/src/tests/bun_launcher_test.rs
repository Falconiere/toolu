use serde_json::Value;

use super::launch;

/// The `@toolu/core/launcher` pair for toolu's old Bun entry: no committed
/// entry uses the Bun launcher anymore, so the golden pins the contract.
const GOLDEN: &str = include_str!("fixtures/bun-session-start.json");

#[test]
fn a_session_start_command_matches_the_bun_launcher_golden() {
  let golden: Value = serde_json::from_str(GOLDEN).unwrap();
  let built = launch("toolu", "SessionStart", "session-start").unwrap();
  assert_eq!(built.command, golden["command"].as_str().unwrap());
  assert_eq!(built.windows, golden["commandWindows"].as_str().unwrap());
}

#[test]
fn an_invalid_entry_is_rejected() {
  let error = launch("demo", "PreToolUse", "Probe").unwrap_err();
  assert!(error.contains("launcher entry must match"), "{error}");
  assert!(error.contains("\"Probe\""), "{error}");
}
