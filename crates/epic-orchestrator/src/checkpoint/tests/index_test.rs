use std::path::Path;

use super::{Meta, Storage, fresh_index, saved_index, short_id, storage};
use crate::support::git;

fn meta(version: u8, index: Option<&str>) -> Meta {
  Meta {
    version,
    worktree: "/w".to_owned(),
    head: "h".to_owned(),
    index: index.map(str::to_owned),
  }
}

#[test]
fn the_saved_index_name_is_trusted_only_when_well_formed() {
  let generation = "index-0123abcd-0000-4000-8000-0123456789ab";
  assert_eq!(saved_index(None), "index");
  assert_eq!(saved_index(Some(&meta(1, Some(generation)))), "index");
  assert_eq!(saved_index(Some(&meta(2, Some(generation)))), generation);
  assert_eq!(saved_index(Some(&meta(2, Some("../escape")))), "index");
  assert_eq!(saved_index(Some(&meta(2, None))), "index");
}

#[test]
fn a_locked_index_starts_a_new_generation_and_another_is_removed() {
  let tmp = tempfile::tempdir().expect("temp");
  let store = Storage {
    path: tmp.path().to_path_buf(),
    worktree: tmp.path().join("w"),
  };
  let index = tmp.path().join("index");
  std::fs::write(&index, "old").expect("index");
  assert_eq!(fresh_index(&store, &index), index);
  assert!(!index.exists());
  std::fs::write(tmp.path().join("index.lock"), "writer").expect("lock");
  let fresh = fresh_index(&store, &index);
  assert_ne!(fresh, index);
  assert!(
    fresh
      .file_name()
      .and_then(|name| name.to_str())
      .is_some_and(|name| name.len() == 42)
  );
}

#[test]
fn a_linked_worktree_keeps_its_index_in_the_common_git_directory() {
  let tmp = tempfile::tempdir().expect("temp");
  let main = tmp.path().join("main");
  crate::support::repo_with_origin(&main, "https://github.com/o/r.git");
  let wt = tmp.path().join("wt");
  git(
    &main,
    &[
      "worktree",
      "add",
      "-q",
      "-b",
      "feat/x",
      &wt.display().to_string(),
    ],
  );
  let found = storage(&wt, None).expect("storage");
  let canonical = std::fs::canonicalize(&wt).expect("canonical");
  let common = std::fs::canonicalize(main.join(".git")).expect("common");
  assert_eq!(found.worktree, canonical);
  assert_eq!(
    found.path,
    common
      .join("toolu")
      .join("checkpoint")
      .join(short_id(&canonical))
  );
  assert!(storage(&tmp.path().join("missing"), None).is_none());
  assert!(storage(tmp.path(), None).is_none());
  assert_ne!(short_id(Path::new("/a")), short_id(Path::new("/b")));
}
