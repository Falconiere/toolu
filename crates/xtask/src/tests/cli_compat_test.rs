use std::path::Path;
use std::process::Command;

use serde_json::Value;

use super::{TREE, run};
use crate::Verdict;
use crate::options::Options;

fn git(root: &Path, args: &[&str]) {
  let ok = Command::new("git")
    .arg("-C")
    .arg(root)
    .args([
      "-c",
      "user.name=t",
      "-c",
      "user.email=t@t",
      "-c",
      "commit.gpgsign=false",
    ])
    .args(args)
    .status()
    .unwrap()
    .success();
  assert!(ok, "git {args:?}");
}

/// The repository's real command tree.
fn real() -> String {
  std::fs::read_to_string(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../docs/cli/commands.json"
  ))
  .unwrap()
}

/// A git repository whose HEAD commits `committed` as the tree (or no tree).
fn repo(committed: Option<&str>) -> tempfile::TempDir {
  let dir = tempfile::tempdir().unwrap();
  git(dir.path(), &["init", "-q"]);
  std::fs::write(dir.path().join("README.md"), "x\n").unwrap();
  if let Some(text) = committed {
    std::fs::create_dir_all(dir.path().join("docs/cli")).unwrap();
    std::fs::write(dir.path().join(TREE), text).unwrap();
  }
  git(dir.path(), &["add", "-A"]);
  git(dir.path(), &["commit", "-q", "-m", "base"]);
  dir
}

fn options(root: &Path, title: Option<&str>) -> Options {
  Options {
    root: std::fs::canonicalize(root).unwrap(),
    base: Some("HEAD".to_owned()),
    title: title.map(str::to_owned),
    ..Options::default()
  }
}

/// The real tree without its `hook` command.
fn without_hook() -> String {
  let mut tree: Value = serde_json::from_str(&real()).unwrap();
  tree["commands"]
    .as_array_mut()
    .unwrap()
    .retain(|command| command["name"] != "hook");
  format!("{tree:#}\n")
}

#[test]
fn an_unchanged_tree_is_compatible() {
  let dir = repo(Some(&real()));
  assert_eq!(run(&options(dir.path(), None)), Ok(Verdict::Clean));
}

#[test]
fn a_removed_command_is_a_finding() {
  let dir = repo(Some(&real()));
  std::fs::write(dir.path().join(TREE), without_hook()).unwrap();
  assert_eq!(
    run(&options(dir.path(), Some("feat(cli): drop hook"))),
    Ok(Verdict::Findings)
  );
}

#[test]
fn a_bumped_hook_protocol_with_a_breaking_title_allows_the_removal() {
  let dir = repo(Some(&real()));
  let bumped = without_hook().replace("\"hookProtocol\": 1", "\"hookProtocol\": 2");
  std::fs::write(dir.path().join(TREE), bumped).unwrap();
  assert_eq!(
    run(&options(dir.path(), Some("feat(cli)!: drop hook"))),
    Ok(Verdict::Clean)
  );
}

#[test]
fn without_a_tree_at_the_base_there_is_nothing_to_compare() {
  let dir = repo(None);
  assert_eq!(run(&options(dir.path(), None)), Ok(Verdict::Clean));
}

#[test]
fn an_unreadable_tree_is_a_setup_error() {
  let dir = repo(Some(&real()));
  std::fs::write(dir.path().join(TREE), "not json").unwrap();
  let err = run(&options(dir.path(), None)).unwrap_err();
  assert!(
    err.starts_with("docs/cli/commands.json at the working tree is not JSON"),
    "{err}"
  );
  std::fs::remove_file(dir.path().join(TREE)).unwrap();
  let err = run(&options(dir.path(), None)).unwrap_err();
  assert!(
    err.starts_with("cannot read docs/cli/commands.json"),
    "{err}"
  );
}
