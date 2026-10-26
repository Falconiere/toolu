use std::path::Path;

use super::{Place, protected_write, publishing};
use crate::tests::git;

fn publishes(root: &Path, cwd: Option<&Path>, command: &str) -> bool {
  let cwd = cwd.map(|cwd| cwd.to_str().unwrap());
  publishing(&Place { root, cwd }, &toolu_shell::analyze(command)).is_some()
}

fn repo() -> tempfile::TempDir {
  let dir = tempfile::tempdir().unwrap();
  git(dir.path(), &["init", "-q"]);
  dir
}

#[test]
fn quote_split_flagged_and_wildcard_publishing_forms_are_caught() {
  let root = repo();
  for command in [
    "git pu\"\"sh origin v9.9.9",
    "gh rel''ease create v9.9.9",
    "gh --repo Falconiere/toolu release create v9.9.9",
    "gh release -R Falconiere/toolu upload v9.9.9 x",
    "gh release new v9.9.9",
    "gh -R x pr edit 1 --add-label \"autorelease: tagged\"",
    "git push origin 'refs/*:refs/*'",
    "git push origin '+refs/*'",
    "git push origin HEAD:refs/notes/x",
    "env -C target/deploy/v9.9.9/tap git push",
  ] {
    assert!(publishes(root.path(), None, command), "allowed: {command}");
  }
  for command in [
    "git push -u origin feature",
    "gh release view v9.9.9",
    "gh -R x pr view 1",
  ] {
    assert!(!publishes(root.path(), None, command), "blocked: {command}");
  }
}

#[test]
fn a_push_from_a_cwd_in_another_repository_is_publishing() {
  let root = repo();
  let tap = root.path().join("target/deploy/v9.9.9/tap");
  std::fs::create_dir_all(&tap).unwrap();
  git(&tap, &["init", "-q"]);
  assert!(publishes(root.path(), Some(&tap), "git push"));
  assert!(!publishes(
    root.path(),
    Some(root.path()),
    "git push -u origin feature"
  ));
}

#[test]
fn remote_config_that_pushes_tags_makes_a_plain_push_publishing() {
  for (key, value) in [
    ("push.followTags", "true"),
    ("remote.origin.mirror", "true"),
    ("remote.origin.push", "refs/tags/*"),
  ] {
    let root = repo();
    assert!(!publishes(root.path(), None, "git push"), "before {key}");
    git(root.path(), &["config", key, value]);
    assert!(publishes(root.path(), None, "git push"), "{key}={value}");
  }
}

#[test]
fn protected_writes_are_found_through_quotes_but_reads_pass() {
  let protected = |path: &str| path.ends_with("target/deploy/gate.json");
  let found = |command: &str| {
    protected_write(&toolu_shell::analyze(command), protected, &["gate.json"]).map(str::to_owned)
  };
  for command in [
    "echo x > target/deploy/gate.js\"\"on",
    "cp /tmp/forged target/deploy/gate.json",
    "R=target/deploy/gate.json; echo x > \"$R\"",
  ] {
    assert!(found(command).is_some(), "missed: {command}");
  }
  for command in [
    "cat target/deploy/gate.json",
    "rg gate.json crates",
    "git commit -m \"guard gate.json writes\"",
    "echo x > \"$OUT\"",
  ] {
    assert_eq!(found(command), None, "{command}");
  }
}
