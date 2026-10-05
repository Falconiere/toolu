use std::path::{Path, PathBuf};
use std::process::Command;

use super::{Workspace, list_files, parts, relative};

const REPO: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");

#[test]
fn the_repository_lists_its_members_innermost_first_and_its_files() {
  let workspace = Workspace::load(&std::fs::canonicalize(REPO).unwrap()).unwrap();
  let member = workspace
    .member_of(Path::new("crates/xtask/src/main.rs"))
    .unwrap();
  assert_eq!(member.name, "xtask");
  assert!(workspace.member_of(Path::new("tooling/src/x.ts")).is_none());
  assert!(workspace.files.contains(&PathBuf::from("Cargo.toml")));
  assert!(
    workspace
      .files_in("crates", "rs")
      .all(|file| file.starts_with("crates"))
  );
  assert!(
    workspace
      .read(Path::new("no-such-file"))
      .unwrap_err()
      .starts_with("cannot read no-such-file")
  );
}

#[test]
fn untracked_files_are_listed_and_ignored_or_deleted_ones_are_not() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path();
  let git = |args: &[&str]| {
    assert!(
      Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .unwrap()
        .status
        .success()
    );
  };
  git(&["init", "-q"]);
  std::fs::write(root.join(".gitignore"), "ignored.txt\n").unwrap();
  std::fs::write(root.join("tracked.txt"), "").unwrap();
  std::fs::write(root.join("gone.txt"), "").unwrap();
  git(&["add", "-A"]);
  std::fs::remove_file(root.join("gone.txt")).unwrap();
  std::fs::write(root.join("new.txt"), "").unwrap();
  std::fs::write(root.join("ignored.txt"), "").unwrap();
  let files = list_files(root).unwrap();
  assert_eq!(
    files,
    [
      PathBuf::from(".gitignore"),
      PathBuf::from("new.txt"),
      PathBuf::from("tracked.txt")
    ]
  );
  assert!(
    list_files(&root.join("missing"))
      .unwrap_err()
      .starts_with("git ls-files failed")
  );
}

#[test]
fn paths_split_and_relativise() {
  assert_eq!(parts(Path::new("a/b/c.rs")), ["a", "b", "c.rs"]);
  assert_eq!(
    relative(Path::new("/r"), Path::new("/r/a/b")),
    PathBuf::from("a/b")
  );
  assert_eq!(
    relative(Path::new("/r"), Path::new("/other")),
    PathBuf::from("/other")
  );
}
