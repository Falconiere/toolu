use std::path::{Path, PathBuf};
use std::process::Command;

use toolu_runtime::env::Env;

use super::{
  base_branch, branch_slug, branch_slugs, common_dir, current_branch, has_git, linked_worktree,
};

fn env() -> Env {
  Env::from_pairs([("PATH", std::env::var("PATH").unwrap())])
}

/// A repository on `feat/x` with one commit, and a linked worktree beside it.
fn repo_with_worktree(dir: &Path) -> (PathBuf, PathBuf) {
  let main = dir.join("main");
  std::fs::create_dir(&main).unwrap();
  for args in [
    &["init", "-q", "-b", "feat/x"][..],
    &[
      "-c",
      "user.name=t",
      "-c",
      "user.email=t@t",
      "commit",
      "-q",
      "--allow-empty",
      "-m",
      "c",
    ],
    &["worktree", "add", "-q", "../wt", "-b", "feat/wt"],
  ] {
    let status = Command::new("git")
      .args(args)
      .current_dir(&main)
      .status()
      .unwrap();
    assert!(status.success(), "git {args:?}");
  }
  (main, dir.join("wt"))
}

#[test]
fn slugs_keep_ascii_words_dashes_and_underscores() {
  assert_eq!(branch_slug("feat/255-state"), "feat_255-state");
  assert_eq!(branch_slug("fix/a.b@c"), "fix_abc");
  assert_eq!(branch_slug("ünïcode/x"), "ncode_x");
  assert_eq!(branch_slug(""), "_default");
  assert_eq!(branch_slug("@@"), "_default");
}

#[test]
fn the_redirected_paths_ask_git_for_the_same_answers() {
  let dir = tempfile::tempdir().unwrap();
  let (main, wt) = repo_with_worktree(dir.path());
  let git_dir = std::fs::canonicalize(main.join(".git")).unwrap();
  let redirected = env().with("GIT_DIR", &git_dir.display().to_string());
  assert_eq!(current_branch(&redirected, &main), "feat/x");
  assert_eq!(current_branch(&env(), &wt), "feat/wt");
  assert!(linked_worktree(&env(), &wt));
  assert!(!linked_worktree(&redirected, &main));
  assert_eq!(common_dir(&redirected, &main), Some(git_dir.clone()));
  assert_eq!(common_dir(&env(), &wt), Some(git_dir));
  let worktree_env = env().with("GIT_CEILING_DIRECTORIES", "/nonexistent");
  assert!(
    linked_worktree(&worktree_env, &wt),
    "deferred, git still says linked"
  );
  assert_eq!(common_dir(&worktree_env, dir.path()), None);
}

#[test]
fn outside_a_repository_the_facts_are_empty() {
  let dir = tempfile::tempdir().unwrap();
  assert_eq!(current_branch(&env(), dir.path()), "");
  assert!(!linked_worktree(&env(), dir.path()));
  assert_eq!(common_dir(&env(), dir.path()), None);
  assert_eq!(base_branch(&env(), None, dir.path()), "main");
  assert_eq!(base_branch(&env(), Some(dir.path()), dir.path()), "main");
  assert!(branch_slugs(&env(), dir.path(), None).is_empty());
}

#[test]
fn base_branch_follows_origin_head_from_the_root_or_the_cwd() {
  let dir = tempfile::tempdir().unwrap();
  let (main, _) = repo_with_worktree(dir.path());
  assert_eq!(base_branch(&env(), Some(&main), dir.path()), "main");
  let origin = main.join(".git/refs/remotes/origin");
  std::fs::create_dir_all(&origin).unwrap();
  std::fs::write(origin.join("HEAD"), "ref: refs/remotes/origin/trunk\n").unwrap();
  assert_eq!(base_branch(&env(), None, &main), "trunk");
  assert_eq!(base_branch(&env(), Some(Path::new("")), &main), "trunk");
  std::fs::write(origin.join("HEAD"), "ref: refs/remotes/upstream/dev\n").unwrap();
  assert_eq!(
    base_branch(&env(), Some(&main), dir.path()),
    "refs/remotes/upstream/dev"
  );
  let redirected = env().with("GIT_CEILING_DIRECTORIES", "/nonexistent");
  assert_eq!(
    base_branch(&redirected, Some(&main), dir.path()),
    "refs/remotes/upstream/dev"
  );
}

#[test]
fn branch_lists_and_git_presence_spawn_git() {
  let dir = tempfile::tempdir().unwrap();
  let (main, _) = repo_with_worktree(dir.path());
  let all: Vec<String> = branch_slugs(&env(), &main, None).into_iter().collect();
  assert_eq!(all, ["feat_wt", "feat_x"]);
  let merged: Vec<String> = branch_slugs(&env(), &main, Some("feat/x"))
    .into_iter()
    .collect();
  assert_eq!(merged, ["feat_wt", "feat_x"]);
  assert!(branch_slugs(&env(), &main, Some("nope")).is_empty());
  assert!(has_git(&env()));
  assert!(!has_git(&Env::from_pairs([("PATH", "")])));
}
