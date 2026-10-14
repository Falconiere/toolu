use std::path::PathBuf;

use serde_json::{Map, Value, json};

use super::{discover, select};

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
