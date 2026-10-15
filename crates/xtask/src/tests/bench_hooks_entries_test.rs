use std::path::PathBuf;

use serde_json::{Map, Value, json};

use super::{HookEntry, discover, select};

fn repo() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn payloads() -> Map<String, Value> {
  let text = std::fs::read_to_string(repo().join("benchmarks/cases/hooks/payloads.json")).unwrap();
  let doc: Value = serde_json::from_str(&text).unwrap();
  doc["entries"].as_object().unwrap().clone()
}

#[test]
fn every_repository_entry_has_exactly_one_payload() {
  let entries = discover(&repo()).unwrap();
  assert!(entries.iter().any(|entry| entry.id == "toolu/pre-tools"));
  let selected = select(entries.clone(), &payloads(), &[]).unwrap();
  assert_eq!(selected, entries);
  let only = select(entries, &payloads(), &["toolu/pre-tools".to_owned()]).unwrap();
  assert_eq!(only.len(), 1);
  assert_eq!(only[0].event, "PreToolUse");
}

#[test]
fn a_stale_or_missing_payload_and_an_unknown_only_are_errors() {
  let entries = discover(&repo()).unwrap();
  let mut extra = payloads();
  extra.insert("x/gone".to_owned(), json!({"stdin": {}}));
  extra.remove("toolu/pre-tools");
  let err = select(entries.clone(), &extra, &[]).unwrap_err();
  assert!(err.contains("no payload for [toolu/pre-tools]"), "{err}");
  assert!(err.contains("unknown entries [x/gone]"), "{err}");
  let err = select(entries, &payloads(), &["x/nope".to_owned()]).unwrap_err();
  assert_eq!(err, "--only names an unknown entry: x/nope");
}

#[test]
fn a_non_native_hook_is_an_error() {
  let dir = tempfile::tempdir().unwrap();
  let hooks = dir.path().join("plugins/x/hooks");
  std::fs::create_dir_all(&hooks).unwrap();
  std::fs::write(
    hooks.join("hooks.json"),
    r#"{"hooks":{"PreToolUse":[{"hooks":[{"command":"bun x.js"}]}]}}"#,
  )
  .unwrap();
  let err = discover(dir.path()).unwrap_err();
  assert!(
    err.ends_with("a PreToolUse hook is not the native launcher"),
    "{err}"
  );
}

#[test]
fn the_measured_spawn_execs_the_hook_directly() {
  let entry = |id: &str| HookEntry {
    id: id.to_owned(),
    event: "PreToolUse".to_owned(),
    root: std::path::PathBuf::from("/p/plugins/x"),
    command: String::new(),
  };
  let bin = std::path::Path::new("/bin/toolu");
  let tail = ["--event", "PreToolUse", "--plugin-root", "/p/plugins/x"];
  let head = ["/bin/sh", "-c", "exec \"$0\" \"$@\"", "/bin/toolu"];
  let core: Vec<&str> = head
    .iter()
    .chain(&["hook", "pre-tools"])
    .chain(&tail)
    .copied()
    .collect();
  assert_eq!(entry("toolu/pre-tools").argv(bin), core);
  let leaf: Vec<&str> = head
    .iter()
    .chain(&["jev", "hook", "session-start"])
    .chain(&tail)
    .copied()
    .collect();
  assert_eq!(entry("jev/session-start").argv(bin), leaf);
}
