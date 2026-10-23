use std::path::Path;

use serde_json::json;

use super::{check, unmapped};

#[test]
fn every_event_in_the_repositorys_hooks_has_a_cursor_name() {
  let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
  let mut seen = 0;
  for entry in std::fs::read_dir(root.join("plugins")).unwrap() {
    let plugin = entry.unwrap().file_name().to_string_lossy().into_owned();
    seen += 1;
    assert_eq!(check(&root, &plugin), Vec::new(), "{plugin}");
  }
  assert!(seen > 5, "plugins directory looks empty: {seen}");
}

#[test]
fn an_event_cursor_lacks_or_nobody_knows_is_reported() {
  let doc = json!({"hooks": {
    "PreToolUse": [], "SessionStart": [], "UserPromptSubmit": [],
    "PermissionRequest": [], "Bogus": []
  }});
  let found = unmapped(&doc);
  let events: Vec<&str> = found.iter().map(|(event, _)| event.as_str()).collect();
  assert_eq!(events, ["Bogus", "PermissionRequest"]);
  assert!(found[1].1.contains("no Cursor event"));
  assert!(found[0].1.contains("any host"));
}

#[test]
fn a_file_without_a_hooks_object_has_nothing_to_report() {
  assert_eq!(unmapped(&json!({"name": "x"})), Vec::new());
}
