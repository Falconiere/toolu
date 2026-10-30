//! The shipped toolu plugin's actual hook inventory and native conformance list.

use std::path::PathBuf;

use toolu_protocol::launcher::{DEFAULT_TIMEOUT, Target, hook as generated_hook};

fn root() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

type Entries = Vec<(&'static str, Option<&'static str>)>;

fn expected() -> [(&'static str, Entries); 5] {
  [
    (
      "SessionStart",
      vec![
        ("session-start", Some("startup|resume|clear|compact")),
        ("check-binary", Some("startup|resume|clear|compact")),
      ],
    ),
    ("PreCompact", vec![("pre-compact", Some("auto"))]),
    ("UserPromptSubmit", vec![("user-prompt-submit", None)]),
    (
      "PreToolUse",
      vec![
        (
          "pre-tools",
          Some("apply_patch|Edit|Write|MultiEdit|Bash|Shell|Grep"),
        ),
        ("mcp-tools", Some("mcp__")),
        ("agent-tier", Some("spawn_agent|Agent|Task")),
      ],
    ),
    (
      "PostToolUse",
      vec![(
        "post-tools",
        Some("apply_patch|Edit|Write|MultiEdit|Bash|Shell"),
      )],
    ),
  ]
}

#[test]
fn every_shipped_toolu_hook_is_native_and_ships_no_bundle() {
  let plugin = root().join("plugins/toolu");
  let text = std::fs::read_to_string(plugin.join("hooks/hooks.json")).unwrap();
  let document: serde_json::Value = serde_json::from_str(&text).unwrap();
  let hooks = document["hooks"].as_object().unwrap();
  let expected = expected();
  assert_eq!(hooks.len(), expected.len());
  assert!(
    !plugin.join("hooks/dist").exists(),
    "#440 removed the Bun bundles"
  );
  for (event, names) in expected {
    let rows = hooks[event].as_array().unwrap();
    assert_eq!(rows.len(), names.len(), "{event}");
    for (row, (name, matcher)) in rows.iter().zip(names) {
      assert_eq!(
        row.get("matcher").and_then(serde_json::Value::as_str),
        matcher
      );
      let native = generated_hook(
        &Target {
          plugin: "toolu",
          event,
          name,
        },
        DEFAULT_TIMEOUT,
      )
      .unwrap();
      let shipped = row["hooks"][0].as_object().unwrap();
      assert_eq!(row["hooks"].as_array().unwrap().len(), 1);
      assert_eq!(shipped["type"], "command");
      assert_eq!(shipped["timeout"], native.timeout);
      assert_eq!(shipped["command"], native.command);
      assert_eq!(shipped["commandWindows"], native.command_windows);
    }
  }
}
