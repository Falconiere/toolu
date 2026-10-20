use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::json;

use super::decide;

fn repo() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn git(root: &Path, args: &[&str]) {
  let status = Command::new("git")
    .args([
      "-c",
      "user.email=ci@example.com",
      "-c",
      "user.name=ci",
      "-C",
    ])
    .arg(root)
    .args(args)
    .status()
    .unwrap();
  assert!(status.success(), "{args:?}");
}

fn rev(root: &Path) -> String {
  let output = Command::new("git")
    .args(["-C"])
    .arg(root)
    .args(["rev-parse", "HEAD"])
    .output()
    .unwrap();
  assert!(output.status.success());
  String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

fn flag(class: &super::super::ci_model::Classification, name: &str) -> Option<bool> {
  class
    .outputs
    .iter()
    .find(|(key, _)| key == name)
    .map(|(_, on)| *on)
}

fn fixture() -> tempfile::TempDir {
  let dir = tempfile::tempdir().unwrap();
  std::fs::create_dir_all(dir.path().join(".github")).unwrap();
  std::fs::copy(
    repo().join(".github/ci-paths.json"),
    dir.path().join(".github/ci-paths.json"),
  )
  .unwrap();
  git(dir.path(), &["init", "-b", "main"]);
  git(dir.path(), &["add", ".github/ci-paths.json"]);
  git(dir.path(), &["commit", "-m", "base"]);
  dir
}

#[test]
fn a_docs_only_file_turns_docs_on_and_rust_off() {
  let dir = fixture();
  let before = rev(dir.path());
  std::fs::create_dir_all(dir.path().join("docs")).unwrap();
  std::fs::write(dir.path().join("docs/install.md"), "hello\n").unwrap();
  git(dir.path(), &["add", "docs/install.md"]);
  git(dir.path(), &["commit", "-m", "docs"]);
  let event = json!({"before": before, "after": rev(dir.path())});
  let class = decide(dir.path(), "push", Some(&event)).unwrap();
  assert_eq!(flag(&class, "docs"), Some(true), "{:?}", class.reasons);
  assert_eq!(flag(&class, "rust"), Some(false), "{:?}", class.reasons);
  assert_eq!(flag(&class, "changed"), Some(true), "{:?}", class.reasons);
}

#[test]
fn a_dispatch_turns_every_group_on() {
  let dir = fixture();
  let class = decide(dir.path(), "workflow_dispatch", None).unwrap();
  assert!(class.outputs.iter().all(|(_, on)| *on));
}

#[test]
fn a_missing_data_file_is_an_error() {
  let empty = tempfile::tempdir().unwrap();
  let err = decide(empty.path(), "push", None).unwrap_err();
  assert!(err.contains("not readable JSON"), "{err}");
}

/// `fixture()` plus `plugins/toolu/.muse-plugin/plugin.json` at version 8.0.4,
/// and the push event of one more commit that rewrites it as `edit` returns.
fn muse_manifest_push(edit: fn(&str) -> String) -> (tempfile::TempDir, serde_json::Value) {
  let dir = fixture();
  let manifest = dir.path().join("plugins/toolu/.muse-plugin/plugin.json");
  std::fs::create_dir_all(manifest.parent().unwrap()).unwrap();
  let base = std::fs::read_to_string(repo().join("plugins/toolu/.muse-plugin/plugin.json"))
    .unwrap()
    .replacen(
      &format!("\"version\": \"{}\"", env!("CARGO_PKG_VERSION")),
      "\"version\": \"8.0.4\"",
      1,
    );
  std::fs::write(&manifest, &base).unwrap();
  git(dir.path(), &["add", "-A"]);
  git(dir.path(), &["commit", "-m", "muse manifest"]);
  let before = rev(dir.path());
  std::fs::write(&manifest, edit(&base)).unwrap();
  git(dir.path(), &["commit", "-am", "edit muse manifest"]);
  let event = json!({"before": before, "after": rev(dir.path())});
  (dir, event)
}

#[test]
fn a_muse_manifest_only_edit_turns_rust_and_ports_on() {
  let (dir, event) = muse_manifest_push(|text| {
    text.replacen("\"timeoutMs\": 60000", "\"timeoutMs\": 30000", 1)
  });
  let class = decide(dir.path(), "push", Some(&event)).unwrap();
  assert_eq!(flag(&class, "rust"), Some(true), "{:?}", class.reasons);
  assert_eq!(flag(&class, "ports"), Some(true), "{:?}", class.reasons);
  assert_eq!(flag(&class, "changed"), Some(true), "{:?}", class.reasons);
}

#[test]
fn a_muse_manifest_version_bump_is_release_only() {
  let (dir, event) = muse_manifest_push(|text| {
    text.replacen("\"version\": \"8.0.4\"", "\"version\": \"8.1.0\"", 1)
  });
  let class = decide(dir.path(), "push", Some(&event)).unwrap();
  assert_eq!(flag(&class, "rust"), Some(false), "{:?}", class.reasons);
  assert_eq!(flag(&class, "ports"), Some(false), "{:?}", class.reasons);
}
