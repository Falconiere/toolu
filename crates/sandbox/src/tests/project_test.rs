use std::path::{Path, PathBuf};

use crate::git_support::git;
use toolu_runtime::env::Env;

use super::{Project, detect, fnv1a, slug};

fn env() -> Env {
  Env::from_pairs([("PATH", "/usr/bin:/bin:/usr/local/bin:/opt/homebrew/bin")])
}

#[test]
fn a_directory_inside_a_checkout_has_its_root_and_relative_path() {
  let dir = tempfile::tempdir().unwrap();
  let root = std::fs::canonicalize(dir.path()).unwrap().join("repo");
  std::fs::create_dir_all(root.join("a/b")).unwrap();
  git(&root, &["init", "-q"]).unwrap();
  let project = detect(&env(), &root.join("a/b"));
  assert_eq!(project.root.as_deref(), Some(root.as_path()));
  assert_eq!(project.rel(), Some(PathBuf::from("a/b")));
  assert_eq!(detect(&env(), &root).rel(), Some(PathBuf::new()));
}

#[test]
fn a_linked_worktree_is_its_own_root() {
  let dir = tempfile::tempdir().unwrap();
  let base = std::fs::canonicalize(dir.path()).unwrap();
  let main = base.join("main");
  std::fs::create_dir_all(&main).unwrap();
  git(&main, &["init", "-q"]).unwrap();
  git(&main, &["commit", "-q", "--allow-empty", "-m", "init"]).unwrap();
  let linked = base.join("linked");
  git(&main, &["worktree", "add", "-q", linked.to_str().unwrap()]).unwrap();
  let project = detect(&env(), &linked);
  assert_eq!(project.root.as_deref(), Some(linked.as_path()));
  assert_eq!(project.common, Some(main.join(".git")));
  assert_eq!(project.mounts(), [linked, main.join(".git")]);
  assert_eq!(detect(&env(), &main).common, None);
}

#[test]
fn outside_git_there_is_no_root() {
  let dir = tempfile::tempdir().unwrap();
  let project = detect(&env(), dir.path());
  assert_eq!(project.root, None);
  assert_eq!(project.rel(), None);
  assert_eq!(project.cwd, std::fs::canonicalize(dir.path()).unwrap());
  let gone = Project {
    cwd: PathBuf::from("/elsewhere"),
    root: Some(PathBuf::from("/repo")),
    common: None,
  };
  assert_eq!(gone.rel(), None);
}

#[test]
fn slugs_are_stable_sanitised_and_unique_per_path() {
  assert_eq!(fnv1a(b""), 0xcbf2_9ce4_8422_2325);
  assert_eq!(fnv1a(b"a"), 0xaf63_dc4c_8601_ec8c);
  let one = slug(Path::new("/w/Toolu Sandbox"));
  assert!(one.starts_with("toolu-sandbox-"), "{one}");
  assert_eq!(one.len(), "toolu-sandbox-".len() + 8);
  assert_eq!(one, slug(Path::new("/w/Toolu Sandbox")));
  assert_ne!(one, slug(Path::new("/x/Toolu Sandbox")));
  assert!(slug(Path::new("/w/café")).starts_with("caf-"));
  assert!(slug(Path::new("/")).starts_with("project-"));
}
