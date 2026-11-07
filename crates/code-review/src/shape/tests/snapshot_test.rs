use std::fs;

use super::review;
use crate::shape::ShapeError;
use crate::shape::tests::{git, isolated_env, repo};

#[test]
fn own_state_through_a_repository_alias_keeps_the_committed_diff_and_index() {
  let dir = repo();
  let parent = tempfile::tempdir().unwrap();
  let alias = parent.path().join("alias");
  std::os::unix::fs::symlink(dir.path(), &alias).unwrap();
  fs::write(dir.path().join("readme.txt"), "committed change\n").unwrap();
  crate::shape::tests::commit(dir.path());
  let state = alias.join(".codex/tmp");
  fs::create_dir_all(state.join("code-review")).unwrap();
  fs::write(state.join("code-review/feature.json"), "private state\n").unwrap();
  let index = fs::read(dir.path().join(".git/index")).unwrap();
  for root in [dir.path(), alias.as_path()] {
    let shaped = review(&isolated_env(), root, Some("main"), &state).unwrap();
    assert!(!shaped.uncommitted, "{root:?}");
    assert_eq!(shaped.gate_files, ["readme.txt"]);
    assert_eq!(
      shaped.diff_sha,
      toolu_state::diff_sha::diff_sha(&isolated_env(), root, "main").unwrap()
    );
    assert_eq!(fs::read(dir.path().join(".git/index")).unwrap(), index);
  }
}

#[test]
fn staged_own_state_is_excluded_without_altering_the_real_index() {
  let dir = repo();
  let root = dir.path();
  let state = root.join(".codex/tmp");
  fs::create_dir_all(state.join("code-review")).unwrap();
  fs::write(state.join("code-review/feature.json"), "private state\n").unwrap();
  git(root, &["add", ".codex/tmp/code-review/feature.json"]);
  fs::write(root.join("readme.txt"), "local change\n").unwrap();
  let index = fs::read(root.join(".git/index")).unwrap();
  let shaped = review(&isolated_env(), root, Some("main"), &state).unwrap();
  assert_eq!(shaped.gate_files, ["readme.txt"]);
  assert!(shaped.files[0].patch.contains("local change"));
  assert_eq!(fs::read(root.join(".git/index")).unwrap(), index);
  assert_eq!(
    git(root, &["diff", "--cached", "--name-only"]),
    ".codex/tmp/code-review/feature.json\n"
  );
}

#[test]
fn staged_own_state_alone_does_not_create_a_review_diff() {
  let dir = repo();
  let root = dir.path();
  let state = root.join(".codex/tmp");
  fs::create_dir_all(state.join("code-review")).unwrap();
  fs::write(state.join("code-review/feature.json"), "private state\n").unwrap();
  git(root, &["add", ".codex/tmp/code-review/feature.json"]);
  let index = fs::read(root.join(".git/index")).unwrap();
  assert!(matches!(
    review(&isolated_env(), root, Some("main"), &state),
    Err(ShapeError::EmptyDiff)
  ));
  assert_eq!(fs::read(root.join(".git/index")).unwrap(), index);
}

#[test]
fn ignored_state_tree_does_not_break_local_capture() {
  let dir = repo();
  let root = dir.path();
  fs::write(root.join(".gitignore"), ".claude/tmp/\n").unwrap();
  git(root, &["add", ".gitignore"]);
  git(root, &["commit", "-m", "ignore state"]);
  let state = root.join(".claude/tmp");
  fs::create_dir_all(state.join("code-review")).unwrap();
  fs::write(state.join("code-review/feature.json"), "private\n").unwrap();
  fs::write(root.join("readme.txt"), "local change\n").unwrap();
  let shaped = review(&isolated_env(), root, Some("main"), &state).unwrap();
  assert!(
    shaped.gate_files.iter().any(|path| path == "readme.txt"),
    "{:?}",
    shaped.gate_files
  );
  assert!(
    shaped
      .files
      .iter()
      .any(|file| file.path == "readme.txt" && file.patch.contains("local change")),
    "{:?}",
    shaped.files
  );
}

#[test]
fn missing_index_can_capture_local_content_without_recreating_it() {
  let dir = repo();
  let root = dir.path();
  let index = root.join(".git/index");
  fs::remove_file(&index).unwrap();
  fs::write(root.join("readme.txt"), "local change\n").unwrap();
  let shaped = review(
    &isolated_env(),
    root,
    Some("main"),
    &root.join(".codex/tmp"),
  )
  .unwrap();
  assert_eq!(shaped.gate_files, ["readme.txt"]);
  assert!(shaped.files[0].patch.contains("local change"));
  assert!(!index.exists());
}
