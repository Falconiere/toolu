use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use toolu_runtime::env::Env;

use super::tree_sha;

fn path_env() -> Env {
  Env::from_pairs([("PATH", std::env::var("PATH").unwrap())])
}

fn git(dir: &Path, args: &[&str]) -> std::process::Output {
  Command::new("git")
    .args(args)
    .current_dir(dir)
    .output()
    .unwrap()
}

fn git_ok(dir: &Path, args: &[&str]) {
  let out = git(dir, args);
  assert!(
    out.status.success(),
    "git {args:?}: {}",
    String::from_utf8_lossy(&out.stderr)
  );
}

fn git_stdout(dir: &Path, args: &[&str]) -> String {
  let out = git(dir, args);
  assert!(
    out.status.success(),
    "git {args:?}: {}",
    String::from_utf8_lossy(&out.stderr)
  );
  String::from_utf8(out.stdout).unwrap().trim().to_owned()
}

/// One commit with `tracked.txt` and a committed `.gitignore` for `*.tmp`.
fn fixture(dir: &Path) {
  git_ok(dir, &["init", "-q", "-b", "main"]);
  fs::write(dir.join("tracked.txt"), "one\n").unwrap();
  fs::write(dir.join(".gitignore"), "*.tmp\n").unwrap();
  git_ok(dir, &["add", "tracked.txt", ".gitignore"]);
  git_ok(
    dir,
    &[
      "-c",
      "user.name=t",
      "-c",
      "user.email=t@t",
      "commit",
      "-q",
      "-m",
      "feat(state): tree_sha fixture",
    ],
  );
}

#[test]
fn unchanged_tree_is_stable() {
  let dir = tempfile::tempdir().unwrap();
  fixture(dir.path());
  let first = tree_sha(&path_env(), dir.path()).expect("hash");
  let second = tree_sha(&path_env(), dir.path()).expect("hash again");
  assert_ne!(first, "");
  assert_eq!(first, second);
}

#[test]
fn tracked_edit_changes_hash() {
  let dir = tempfile::tempdir().unwrap();
  fixture(dir.path());
  let before = tree_sha(&path_env(), dir.path()).expect("before");
  fs::write(dir.path().join("tracked.txt"), "two\n").unwrap();
  let after = tree_sha(&path_env(), dir.path()).expect("after");
  assert_ne!(before, after);
}

#[test]
fn untracked_file_changes_hash() {
  let dir = tempfile::tempdir().unwrap();
  fixture(dir.path());
  let before = tree_sha(&path_env(), dir.path()).expect("before");
  fs::write(dir.path().join("extra.txt"), "extra\n").unwrap();
  let after = tree_sha(&path_env(), dir.path()).expect("after");
  assert_ne!(before, after);
}

#[test]
fn ignored_file_does_not_change_hash() {
  let dir = tempfile::tempdir().unwrap();
  fixture(dir.path());
  let before = tree_sha(&path_env(), dir.path()).expect("before");
  fs::write(dir.path().join("scratch.tmp"), "ignored\n").unwrap();
  let after = tree_sha(&path_env(), dir.path()).expect("after");
  assert_eq!(before, after);
}

#[test]
fn real_index_and_porcelain_unchanged() {
  let dir = tempfile::tempdir().unwrap();
  fixture(dir.path());
  fs::write(dir.path().join("tracked.txt"), "dirty\n").unwrap();
  fs::write(dir.path().join("extra.txt"), "extra\n").unwrap();
  let before_porc = git_stdout(dir.path(), &["status", "--porcelain"]);
  let before_index = fs::read(dir.path().join(".git/index")).unwrap();
  assert!(tree_sha(&path_env(), dir.path()).is_some());
  assert_eq!(
    git_stdout(dir.path(), &["status", "--porcelain"]),
    before_porc
  );
  assert_eq!(
    fs::read(dir.path().join(".git/index")).unwrap(),
    before_index
  );
}

#[test]
fn missing_repo_or_git_returns_none() {
  let empty = tempfile::tempdir().unwrap();
  assert_eq!(tree_sha(&path_env(), empty.path()), None);

  let dir = tempfile::tempdir().unwrap();
  fixture(dir.path());
  assert_eq!(
    tree_sha(&Env::from_pairs([("PATH", "")]), dir.path()),
    None,
    "no git"
  );
}

#[test]
fn write_tree_matches_manual_temp_index_pipeline() {
  let dir = tempfile::tempdir().unwrap();
  fixture(dir.path());
  fs::write(dir.path().join("extra.txt"), "extra\n").unwrap();
  // Keep the temp index outside the worktree so `git add -A` does not stage it.
  let scratch = tempfile::tempdir().unwrap();
  let idx: PathBuf = scratch.path().join("index");
  let idx_s = idx.display().to_string();
  let mut read = Command::new("git");
  read
    .args(["read-tree", "HEAD"])
    .current_dir(dir.path())
    .env("GIT_INDEX_FILE", &idx_s);
  assert!(read.status().unwrap().success());
  let mut add = Command::new("git");
  add
    .args(["add", "-A"])
    .current_dir(dir.path())
    .env("GIT_INDEX_FILE", &idx_s);
  assert!(add.status().unwrap().success());
  let mut write = Command::new("git");
  write
    .args(["write-tree"])
    .current_dir(dir.path())
    .env("GIT_INDEX_FILE", &idx_s);
  let out = write.output().unwrap();
  assert!(out.status.success());
  let piped = String::from_utf8(out.stdout).unwrap().trim().to_owned();
  let ours = tree_sha(&path_env(), dir.path()).expect("hash");
  assert_eq!(ours, piped);
}
