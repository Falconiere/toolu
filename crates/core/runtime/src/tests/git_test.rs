use std::path::{Path, PathBuf};
use std::process::Command;

use super::{Discovery, Repo, discover, toplevel};
use crate::env::Env;

fn env() -> Env {
  Env::from_pairs([
    ("PATH", std::env::var("PATH").unwrap()),
    ("HOME", std::env::temp_dir().display().to_string()),
  ])
}

fn run_git(cwd: &Path, args: &[&str]) -> String {
  let out = Command::new("git")
    .args([
      "-c",
      "init.defaultBranch=main",
      "-c",
      "user.name=t",
      "-c",
      "user.email=t@t",
    ])
    .args(args)
    .current_dir(cwd)
    .env("GIT_CONFIG_NOSYSTEM", "1")
    .output()
    .unwrap();
  assert!(
    out.status.success(),
    "git {args:?}: {}",
    String::from_utf8_lossy(&out.stderr)
  );
  String::from_utf8(out.stdout).unwrap().trim_end().to_owned()
}

fn real(path: &Path) -> PathBuf {
  std::fs::canonicalize(path).unwrap()
}

fn repo_at(top: &Path) -> Repo {
  let git = real(top).join(".git");
  Repo {
    toplevel: Some(real(top)),
    git_dir: git.clone(),
    common_dir: git,
  }
}

#[test]
fn a_repository_is_found_from_its_root_and_a_symlinked_subdirectory() {
  let dir = tempfile::tempdir().unwrap();
  let top = dir.path().join("repo");
  std::fs::create_dir_all(top.join("a/b")).unwrap();
  run_git(&top, &["init", "-q"]);
  std::os::unix::fs::symlink(top.join("a/b"), dir.path().join("link")).unwrap();
  assert_eq!(discover(&env(), &top), Discovery::Repo(repo_at(&top)));
  assert_eq!(
    discover(&env(), &dir.path().join("link")),
    Discovery::Repo(repo_at(&top))
  );
  assert_eq!(toplevel(&env(), &dir.path().join("link")), Some(real(&top)));
}

#[test]
fn a_linked_worktree_has_its_own_git_dir_and_the_main_common_dir() {
  let dir = tempfile::tempdir().unwrap();
  let main = dir.path().join("main");
  std::fs::create_dir(&main).unwrap();
  run_git(&main, &["init", "-q"]);
  run_git(
    &main,
    &["commit", "-q", "--allow-empty", "-m", "c", "--no-gpg-sign"],
  );
  run_git(&main, &["worktree", "add", "-q", "../wt", "-b", "feat/wt"]);
  let wt = dir.path().join("wt");
  let Discovery::Repo(found) = discover(&env(), &wt) else {
    panic!("no repository");
  };
  assert_eq!(found.toplevel, Some(real(&wt)));
  assert_eq!(found.git_dir, real(&main.join(".git/worktrees/wt")));
  assert_eq!(found.common_dir, real(&main.join(".git")));
}

#[test]
fn bare_repositories_and_git_dirs_have_no_toplevel() {
  let dir = tempfile::tempdir().unwrap();
  let bare = dir.path().join("b.git");
  std::fs::create_dir(&bare).unwrap();
  run_git(&bare, &["init", "-q", "--bare"]);
  let Discovery::Repo(found) = discover(&env(), &bare.join("refs")) else {
    panic!("no repository");
  };
  assert_eq!((found.toplevel, found.git_dir), (None, real(&bare)));
  let work = dir.path().join("w");
  std::fs::create_dir(&work).unwrap();
  run_git(&work, &["init", "-q"]);
  assert_eq!(toplevel(&env(), &work.join(".git/objects")), None);
}

#[test]
fn a_separate_git_dir_pointer_is_followed() {
  let dir = tempfile::tempdir().unwrap();
  let work = dir.path().join("w");
  std::fs::create_dir(&work).unwrap();
  run_git(&work, &["init", "-q", "--separate-git-dir", "../store"]);
  let Discovery::Repo(found) = discover(&env(), &work) else {
    panic!("no repository");
  };
  assert_eq!(found.toplevel, Some(real(&work)));
  assert_eq!(found.git_dir, real(&dir.path().join("store")));
}

#[test]
fn an_invalid_gitfile_ends_the_walk_and_an_invalid_git_dir_is_skipped() {
  let dir = tempfile::tempdir().unwrap();
  let outer = dir.path().join("outer");
  std::fs::create_dir_all(outer.join("inner/deep")).unwrap();
  run_git(&outer, &["init", "-q"]);
  std::fs::create_dir(outer.join("inner/.git")).unwrap();
  assert_eq!(
    toplevel(&env(), &outer.join("inner/deep")),
    Some(real(&outer))
  );
  std::fs::remove_dir(outer.join("inner/.git")).unwrap();
  std::fs::write(outer.join("inner/.git"), "nonsense\n").unwrap();
  assert_eq!(
    discover(&env(), &outer.join("inner/deep")),
    Discovery::NotFound
  );
  assert_eq!(
    discover(&env(), &dir.path().join("missing")),
    Discovery::NotFound
  );
  assert_eq!(toplevel(&env(), dir.path()), None);
}

#[test]
fn deferred_cases_ask_git_and_get_its_answer() {
  let dir = tempfile::tempdir().unwrap();
  let top = dir.path().join("repo");
  let elsewhere = dir.path().join("elsewhere");
  std::fs::create_dir_all(&elsewhere).unwrap();
  std::fs::create_dir(&top).unwrap();
  run_git(&top, &["init", "-q"]);
  assert_eq!(
    discover(&env().with("GIT_DIR", "x"), &top),
    Discovery::AskGit
  );
  run_git(
    &top,
    &["config", "core.worktree", &elsewhere.display().to_string()],
  );
  assert_eq!(discover(&env(), &top), Discovery::AskGit);
  assert_eq!(toplevel(&env(), &top), Some(real(&elsewhere)));
}
