use std::path::{Path, PathBuf};

use serde_json::Value;

use super::{Finding, check};
use crate::print_hook::entry_json;
use toolu_protocol::launcher::{Target, hook};

const WIRING: &[&str] = &[
  "hooks/hooks.json",
  ".claude-plugin/plugin.json",
  ".codex-plugin/plugin.json",
];

fn repo() -> PathBuf {
  PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
}

/// A copy of the repository's plugin wiring: every hooks.json and manifest.
fn copy_plugins() -> tempfile::TempDir {
  let dir = tempfile::tempdir().unwrap();
  for plugin in std::fs::read_dir(repo().join("plugins")).unwrap() {
    let plugin = plugin.unwrap().path();
    for file in WIRING {
      let from = plugin.join(file);
      if from.is_file() {
        let to = dir
          .path()
          .join("plugins")
          .join(plugin.file_name().unwrap())
          .join(file);
        std::fs::create_dir_all(to.parent().unwrap()).unwrap();
        std::fs::copy(&from, &to).unwrap();
      }
    }
    let dist = plugin.join("hooks/dist");
    if dist.is_dir() {
      let to = dir
        .path()
        .join("plugins")
        .join(plugin.file_name().unwrap())
        .join("hooks/dist");
      copy_tree(&dist, &to);
    }
  }
  dir
}

fn copy_tree(from: &Path, to: &Path) {
  std::fs::create_dir_all(to).unwrap();
  for entry in std::fs::read_dir(from).unwrap() {
    let entry = entry.unwrap();
    let dest = to.join(entry.file_name());
    if entry.path().is_dir() {
      copy_tree(&entry.path(), &dest);
    } else {
      std::fs::copy(entry.path(), dest).unwrap();
    }
  }
}

/// Add the generated native toolu `PreToolUse` entry, edited by `edit`.
fn add_native(root: &Path, edit: impl Fn(&mut Value)) -> String {
  let path = root.join("plugins/toolu/hooks/hooks.json");
  let mut json: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
  let target = Target {
    plugin: "toolu",
    event: "PreToolUse",
    name: "pre-tools",
  };
  let mut entry: Value = serde_json::from_str(&entry_json(&hook(&target, 60).unwrap())).unwrap();
  edit(&mut entry);
  let text = entry.to_string();
  let group = serde_json::json!({ "matcher": "Bash", "hooks": [entry] });
  json["hooks"]["PreToolUse"]
    .as_array_mut()
    .unwrap()
    .push(group);
  std::fs::write(&path, serde_json::to_string_pretty(&json).unwrap()).unwrap();
  text
}

fn set_versions(root: &Path, version: &str) {
  for manifest in glob(root) {
    let mut json: Value =
      serde_json::from_str(&std::fs::read_to_string(&manifest).unwrap()).unwrap();
    json["version"] = Value::from(version);
    std::fs::write(&manifest, json.to_string()).unwrap();
  }
}

fn glob(root: &Path) -> Vec<PathBuf> {
  let mut found = Vec::new();
  for plugin in std::fs::read_dir(root.join("plugins")).unwrap() {
    for dir in [".claude-plugin", ".codex-plugin"] {
      let path = plugin
        .as_ref()
        .unwrap()
        .path()
        .join(dir)
        .join("plugin.json");
      if path.is_file() {
        found.push(path);
      }
    }
  }
  found
}

#[test]
fn the_repository_is_clean() {
  assert_eq!(check(&repo()).unwrap(), Vec::<Finding>::new());
}

#[test]
fn a_generated_native_entry_survives_version_bumps() {
  let dir = copy_plugins();
  let entry = add_native(dir.path(), |_| {});
  assert_eq!(check(dir.path()).unwrap(), Vec::<Finding>::new());
  for version in ["7.11.0", "8.0.0"] {
    set_versions(dir.path(), version);
    assert_eq!(
      check(dir.path()).unwrap(),
      Vec::<Finding>::new(),
      "{version}"
    );
    let text = std::fs::read_to_string(dir.path().join("plugins/toolu/hooks/hooks.json")).unwrap();
    let json: Value = serde_json::from_str(&text).unwrap();
    let last = json["hooks"]["PreToolUse"]
      .as_array()
      .unwrap()
      .last()
      .unwrap();
    assert_eq!(last["hooks"][0].to_string(), entry);
  }
}

#[test]
fn a_hand_edited_native_entry_is_named_with_the_expected_command() {
  let dir = copy_plugins();
  add_native(dir.path(), |entry| {
    let command = entry["command"]
      .as_str()
      .unwrap()
      .replace("exit 2", "exit 0");
    entry["command"] = Value::from(command);
  });
  let found = check(dir.path()).unwrap();
  assert_eq!(found.len(), 1, "{found:?}");
  assert_eq!(found[0].file, "plugins/toolu/hooks/hooks.json");
  assert!(found[0].at.starts_with("PreToolUse["));
  let shown = found[0].to_string();
  assert!(shown.contains(": command differs from the generated launcher\n  expected: t=;"));
}

#[test]
fn a_missing_timeout_and_a_manifest_problem_are_both_found() {
  let dir = copy_plugins();
  add_native(dir.path(), |entry| {
    entry.as_object_mut().unwrap().remove("timeout");
  });
  let manifest = dir.path().join("plugins/jev/.claude-plugin/plugin.json");
  let mut json: Value = serde_json::from_str(&std::fs::read_to_string(&manifest).unwrap()).unwrap();
  json["hookProtocol"] = Value::from(2);
  std::fs::write(&manifest, json.to_string()).unwrap();
  let shown: Vec<String> = check(dir.path())
    .unwrap()
    .iter()
    .map(ToString::to_string)
    .collect();
  assert_eq!(shown.len(), 2, "{shown:?}");
  assert!(
    shown[0].ends_with("hookProtocol is 2, but toolu speaks 1"),
    "{shown:?}"
  );
  assert!(shown[0].starts_with("check-hooks: plugins/jev/.claude-plugin/plugin.json: "));
  assert!(
    shown[1].contains("timeout must be an integer from 1 to 600"),
    "{shown:?}"
  );
}

#[test]
fn invalid_hooks_json_and_a_missing_plugins_dir_are_reported() {
  let dir = copy_plugins();
  std::fs::write(dir.path().join("plugins/toolu/hooks/hooks.json"), "{").unwrap();
  let found = check(dir.path()).unwrap();
  assert!(found[0].problem.starts_with("invalid JSON"), "{found:?}");
  std::fs::write(dir.path().join("plugins/toolu/hooks/hooks.json"), "{}").unwrap();
  assert_eq!(check(dir.path()).unwrap()[0].problem, "no \"hooks\" object");
  let empty = tempfile::tempdir().unwrap();
  assert!(check(empty.path()).unwrap_err().starts_with("cannot list"));
}
