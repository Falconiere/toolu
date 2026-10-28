use std::path::PathBuf;

use super::{covers, missing, plan};

fn paths(items: &[&str]) -> Vec<PathBuf> {
  items.iter().map(PathBuf::from).collect()
}

#[test]
fn a_plan_is_sorted_unique_and_drops_what_an_ancestor_holds() {
  let planned = plan(paths(&["/w/b", "/w", "/x/y", "/w/a/c", "/x/y", "/x/yz"]));
  assert_eq!(planned, paths(&["/w", "/x/y", "/x/yz"]));
  assert_eq!(plan(Vec::new()), Vec::<PathBuf>::new());
}

#[test]
fn a_mount_covers_itself_and_what_it_holds_but_not_a_sibling_prefix() {
  let mounts = paths(&["/repo", "/main/.git"]);
  assert!(covers(&mounts, &PathBuf::from("/repo")));
  assert!(covers(&mounts, &PathBuf::from("/repo/sub/dir")));
  assert!(!covers(&mounts, &PathBuf::from("/repository")));
  assert_eq!(
    missing(&mounts, &paths(&["/repo/x", "/main/.git", "/elsewhere"])),
    paths(&["/elsewhere"])
  );
}
