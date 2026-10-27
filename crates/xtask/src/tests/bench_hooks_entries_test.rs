use std::path::PathBuf;

use serde_json::{Map, Value, json};

use super::{HookEntry, Launch, discover, select};

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
    launch: Launch::Command(String::new()),
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

#[test]
fn every_muse_manifest_hook_is_an_entry_after_the_hooks_json_ones() {
  let entries = discover(&repo()).unwrap();
  let muse: Vec<&HookEntry> = entries
    .iter()
    .filter(|entry| entry.id.starts_with("muse:"))
    .collect();
  assert_eq!(muse.len(), 28, "{muse:?}");
  let first_muse = entries
    .iter()
    .position(|entry| entry.id.starts_with("muse:"))
    .unwrap();
  assert!(
    entries[first_muse..]
      .iter()
      .all(|entry| entry.id.starts_with("muse:"))
  );
  let pre = muse
    .iter()
    .find(|entry| entry.id == "muse:toolu/pre-tools")
    .unwrap();
  assert_eq!(pre.event, "PreToolUse");
  assert_eq!(
    pre.launch,
    Launch::Resolver(repo().join("plugins/toolu/hooks/muse/pre-tools"))
  );
  assert_eq!(pre.plugin(), "toolu");
}

#[test]
fn a_muse_entry_spawns_its_resolver_with_the_shell_muse_names() {
  let entry = HookEntry {
    id: "muse:jev/session-start".to_owned(),
    event: "SessionStart".to_owned(),
    root: std::path::PathBuf::from("/p/plugins/jev"),
    launch: Launch::Resolver(std::path::PathBuf::from(
      "/p/plugins/jev/hooks/muse/session-start",
    )),
  };
  assert_eq!(
    entry.argv(std::path::Path::new("/bin/toolu")),
    ["/bin/sh", "/p/plugins/jev/hooks/muse/session-start"]
  );
  assert_eq!(entry.plugin(), "jev");
}

fn muse_manifest(text: &str) -> tempfile::TempDir {
  let dir = tempfile::tempdir().unwrap();
  let manifest = dir.path().join("plugins/x/.muse-plugin");
  std::fs::create_dir_all(&manifest).unwrap();
  std::fs::write(manifest.join("plugin.json"), text).unwrap();
  dir
}

#[test]
fn a_muse_hook_that_is_not_the_generated_resolver_is_an_error() {
  let dir = muse_manifest(
    r#"{"capabilities":{"hooks":[{"id":"pre-tools","event":"PreToolUse","command":["toolu","hook","pre-tools"]}]}}"#,
  );
  let err = discover(dir.path()).unwrap_err();
  assert!(
    err.ends_with("Muse hook pre-tools is not /bin/sh hooks/muse/pre-tools"),
    "{err}"
  );
}

#[test]
fn an_unreadable_muse_manifest_names_the_file() {
  let dir = muse_manifest("{not json");
  let err = discover(dir.path()).unwrap_err();
  assert!(err.contains("plugins/x/.muse-plugin/plugin.json"), "{err}");
  let none = muse_manifest(r#"{"capabilities":{"skills":[]}}"#);
  assert_eq!(discover(none.path()).unwrap(), []);
}
