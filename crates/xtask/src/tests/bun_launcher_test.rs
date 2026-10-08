use std::path::PathBuf;

use serde_json::Value;

use super::launch;

fn repo() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn a_session_start_command_matches_the_committed_entry() {
  let path = repo().join("plugins/epic-orchestrator/hooks/hooks.json");
  let text = std::fs::read_to_string(&path).unwrap();
  let json: Value = serde_json::from_str(&text).unwrap();
  let hook = &json["hooks"]["SessionStart"][0]["hooks"][0];
  let built = launch("epic-orchestrator", "SessionStart", "check-deps").unwrap();
  assert_eq!(built.command, hook["command"].as_str().unwrap());
  assert_eq!(built.windows, hook["commandWindows"].as_str().unwrap());
}

#[test]
fn an_invalid_entry_is_rejected() {
  let error = launch("demo", "PreToolUse", "Probe").unwrap_err();
  assert!(error.contains("launcher entry must match"), "{error}");
  assert!(error.contains("\"Probe\""), "{error}");
}
