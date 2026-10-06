use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use toolu_protocol::host::Host;

use super::{PermissionsResult, permissions_autowrite};
use crate::config::load::LoadedConfig;
use crate::env::Env;

fn config(data: Value, host: Host) -> LoadedConfig {
  let Value::Object(map) = data else {
    panic!("not an object")
  };
  LoadedConfig::from_data(map, host)
}

/// A real git repository and an environment whose git stops at its parent.
fn repo() -> (tempfile::TempDir, PathBuf, Env) {
  let dir = tempfile::tempdir().unwrap();
  let root = std::fs::canonicalize(dir.path()).unwrap().join("repo");
  std::fs::create_dir(&root).unwrap();
  let status = std::process::Command::new("git")
    .args(["init", "-q"])
    .current_dir(&root)
    .status()
    .unwrap();
  assert!(status.success());
  let env = Env::from_pairs([("PATH", std::env::var("PATH").unwrap())]).with(
    "GIT_CEILING_DIRECTORIES",
    &root.parent().unwrap().display().to_string(),
  );
  (dir, root, env)
}

fn settings(root: &Path) -> PathBuf {
  root.join(".claude/settings.local.json")
}

#[test]
fn a_fresh_repository_gets_the_default_allowlist_once() {
  let (_dir, root, env) = repo();
  let c = config(json!({}), Host::Claude);
  let file = settings(&root);
  let notice = format!(
    "toolu wrote Bash(*), Edit, Write to {} (one time only; delete a rule and it stays deleted). Add that file to .gitignore if it is not there already.",
    file.display()
  );
  let added = vec!["Bash(*)".to_owned(), "Edit".to_owned(), "Write".to_owned()];
  let expected = PermissionsResult::Written {
    settings_file: file.clone(),
    added,
    notice: Some(notice),
  };
  assert_eq!(permissions_autowrite(&c, &env, Some(&root), None), expected);
  let text = std::fs::read_to_string(&file).unwrap();
  assert_eq!(
    text,
    "{\n  \"permissions\": {\n    \"allow\": [\n      \"Bash(*)\",\n      \"Edit\",\n      \"Write\"\n    ]\n  }\n}\n"
  );
  assert!(root.join(".claude/tmp/.permissions-written").is_file());
  let again = permissions_autowrite(&c, &env, Some(&root), None);
  assert_eq!(
    again,
    PermissionsResult::Skipped("already written".to_owned())
  );
}

#[test]
fn existing_settings_keep_their_keys_in_order_and_only_missing_rules_are_added() {
  let (_dir, root, env) = repo();
  let file = settings(&root);
  std::fs::create_dir_all(file.parent().unwrap()).unwrap();
  std::fs::write(
    &file,
    r#"{"z":1,"permissions":{"deny":["x"],"allow":["Edit",7]},"a":{"y":1,"b":2}}"#,
  )
  .unwrap();
  let c = config(
    json!({ "permissions": { "allow": ["Edit", "Read", 3] } }),
    Host::Claude,
  );
  let PermissionsResult::Written { added, .. } = permissions_autowrite(&c, &env, Some(&root), None)
  else {
    panic!("not written");
  };
  assert_eq!(added, ["Read"]);
  let text = std::fs::read_to_string(&file).unwrap();
  let compact: String = text.split_whitespace().collect();
  assert_eq!(
    compact,
    r#"{"z":1,"permissions":{"deny":["x"],"allow":["Edit","Read"]},"a":{"y":1,"b":2}}"#
  );
}

#[test]
fn an_allow_object_contributes_its_string_values() {
  let (_dir, root, env) = repo();
  let file = settings(&root);
  std::fs::create_dir_all(file.parent().unwrap()).unwrap();
  std::fs::write(&file, r#"{"permissions":{"allow":{"k":"Edit","n":5}}}"#).unwrap();
  let result = permissions_autowrite(&config(json!({}), Host::Claude), &env, Some(&root), None);
  let PermissionsResult::Written { added, notice, .. } = result else {
    panic!("not written")
  };
  assert_eq!(added, ["Bash(*)", "Write"]);
  assert!(notice.is_some());
}

#[test]
fn an_unparsable_or_unmergeable_file_is_left_byte_identical() {
  for (text, reason) in [
    ("{\"permissions\":", "malformed JSON in"),
    ("null", "malformed JSON in"),
    ("[1]", "could not merge"),
    (r#"{"permissions":{"allow":"Edit"}}"#, "could not merge"),
    (r#"{"permissions":true}"#, "could not merge"),
  ] {
    let (_dir, root, env) = repo();
    let file = settings(&root);
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, text).unwrap();
    let c = config(json!({}), Host::Claude);
    let PermissionsResult::Skipped(got) = permissions_autowrite(&c, &env, Some(&root), None) else {
      panic!("written over {text}");
    };
    assert!(got.starts_with(reason), "{text}: {got}");
    assert_eq!(c.take_warnings(), [got]);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), text);
    assert!(!root.join(".claude/tmp/.permissions-written").exists());
  }
}

#[test]
fn each_precondition_skips_with_its_reason() {
  let (_dir, root, env) = repo();
  let skip = |c: &LoadedConfig, env: &Env, root: Option<&Path>| match permissions_autowrite(
    c, env, root, None,
  ) {
    PermissionsResult::Skipped(reason) => reason,
    PermissionsResult::Written { .. } => panic!("written"),
  };
  assert_eq!(
    skip(&config(json!({}), Host::Codex), &env, Some(&root)),
    "not claude"
  );
  let mut invalid = config(json!({}), Host::Claude);
  invalid.invalid = Some("x: unknown top-level key 'nope'".to_owned());
  assert_eq!(skip(&invalid, &env, Some(&root)), "config invalid");
  let off = config(
    json!({ "permissions": { "autoAllow": false } }),
    Host::Claude,
  );
  assert_eq!(skip(&off, &env, Some(&root)), "autoAllow is false");
  let plain = root.parent().unwrap().join("plain");
  std::fs::create_dir(&plain).unwrap();
  assert_eq!(
    skip(&config(json!({}), Host::Claude), &env, Some(&plain)),
    "not a git repository"
  );
  let none = Env::from_pairs([("TOOLU_PROJECT_DIR", "")]);
  let empty = Path::new("");
  assert_eq!(
    skip(&config(json!({}), Host::Claude), &none, Some(empty)),
    "no project root"
  );
}

#[test]
fn an_unwritable_settings_path_is_reported_and_retried_next_session() {
  let (_dir, root, env) = repo();
  std::fs::write(root.join(".claude"), "a file where the directory goes").unwrap();
  let c = config(json!({}), Host::Claude);
  let reason = format!("could not write {}", settings(&root).display());
  assert_eq!(
    permissions_autowrite(&c, &env, Some(&root), None),
    PermissionsResult::Skipped(reason.clone())
  );
  assert_eq!(c.take_warnings(), [reason]);
}
