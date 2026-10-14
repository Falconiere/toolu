use std::path::PathBuf;

use super::{builtin_gates, hook_module};

#[test]
fn a_native_command_is_named_by_its_hook() {
  let command = "t=; toolu --hook-protocol; \"$t\" jev hook session-start --event SessionStart";
  assert_eq!(hook_module(command).as_deref(), Some("hook session-start"));
  assert_eq!(
    hook_module("bun \"${CLAUDE_PLUGIN_ROOT}/hooks/dist/x.js\""),
    None
  );
}

#[test]
fn the_engine_tables_list_their_gate_modules_in_order() {
  let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
  let post = builtin_gates(&root, "POST_TOOL").unwrap();
  assert_eq!(
    post,
    [
      (
        "gate-status".to_owned(),
        "crates/core/engine/src/gates/gate_status.rs".to_owned()
      ),
      (
        "push-waiver".to_owned(),
        "crates/core/engine/src/gates/push_waiver.rs".to_owned()
      ),
    ]
  );
  assert!(
    builtin_gates(&root, "NO_TABLE")
      .unwrap_err()
      .contains("NO_TABLE missing")
  );
}
