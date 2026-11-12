use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use toolu_runtime::env::Env;

use super::tree_sha;

fn env() -> Env {
  Env::from_pairs([("PATH", std::env::var("PATH").unwrap())])
}

fn sh(dir: &Path, script: &str) -> String {
  let out = Command::new("sh")
    .args(["-c", script])
    .current_dir(dir)
    .output()
    .unwrap();
  assert!(
    out.status.success(),
    "{script}: {}",
    String::from_utf8_lossy(&out.stderr)
  );
  String::from_utf8(out.stdout).unwrap().trim().to_owned()
}

/// One commit with `tracked.txt` and a committed `.gitignore` for `*.tmp`.
fn fixture(dir: &Path) {
  sh(
    dir,
    "git init -q -b main \
     && printf 'one\\n' > tracked.txt \
     && printf '*.tmp\\n' > .gitignore \
     && git add tracked.txt .gitignore \
     && git -c user.name=t -c user.email=t@t commit -q -m 'feat(state): tree_sha fixture'",
  );
}

fn porcelain(dir: &Path) -> String {
  sh(dir, "git status --porcelain")
}

fn index_bytes(dir: &Path) -> Vec<u8> {
  fs::read(dir.join(".git/index")).unwrap()
}

#[test]
fn unchanged_tree_is_stable() {
  let dir = tempfile::tempdir().unwrap();
  fixture(dir.path());
  let first = tree_sha(&env(), dir.path()).expect("hash");
  let second = tree_sha(&env(), dir.path()).expect("hash again");
  assert!(!first.is_empty());
  assert_eq!(first, second);
}

#[test]
fn tracked_edit_changes_hash() {
  let dir = tempfile::tempdir().unwrap();
  fixture(dir.path());
  let before = tree_sha(&env(), dir.path()).expect("before");
  fs::write(dir.path().join("tracked.txt"), "two\n").unwrap();
  let after = tree_sha(&env(), dir.path()).expect("after");
  assert_ne!(before, after);
}

#[test]
fn untracked_file_changes_hash() {
  let dir = tempfile::tempdir().unwrap();
  fixture(dir.path());
  let before = tree_sha(&env(), dir.path()).expect("before");
  fs::write(dir.path().join("extra.txt"), "extra\n").unwrap();
  let after = tree_sha(&env(), dir.path()).expect("after");
  assert_ne!(before, after);
}

#[test]
fn ignored_file_does_not_change_hash() {
  let dir = tempfile::tempdir().unwrap();
  fixture(dir.path());
  let before = tree_sha(&env(), dir.path()).expect("before");
  fs::write(dir.path().join("scratch.tmp"), "ignored\n").unwrap();
  let after = tree_sha(&env(), dir.path()).expect("after");
  assert_eq!(before, after);
}

#[test]
fn real_index_and_porcelain_unchanged() {
  let dir = tempfile::tempdir().unwrap();
  fixture(dir.path());
  fs::write(dir.path().join("tracked.txt"), "dirty\n").unwrap();
  fs::write(dir.path().join("extra.txt"), "extra\n").unwrap();
  let before_porc = porcelain(dir.path());
  let before_index = index_bytes(dir.path());
  assert!(tree_sha(&env(), dir.path()).is_some());
  assert_eq!(porcelain(dir.path()), before_porc);
  assert_eq!(index_bytes(dir.path()), before_index);
}

#[test]
fn missing_repo_or_git_returns_none() {
  let empty = tempfile::tempdir().unwrap();
  assert_eq!(tree_sha(&env(), empty.path()), None);

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
  let piped = sh(
    dir.path(),
    &format!(
      "GIT_INDEX_FILE={idx} git read-tree HEAD \
       && GIT_INDEX_FILE={idx} git add -A \
       && GIT_INDEX_FILE={idx} git write-tree",
      idx = idx.display()
    ),
  );
  let ours = tree_sha(&env(), dir.path()).expect("hash");
  assert_eq!(ours, piped);
}
