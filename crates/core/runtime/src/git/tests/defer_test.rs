use std::path::Path;

use super::{config_defers, env_defers, foreign_owner, moves_worktree, truthy};
use crate::env::Env;

#[test]
fn redirecting_variables_defer_and_empty_ones_do_not() {
  let base = Env::from_pairs([("PATH", "/usr/bin"), ("HOME", "/h")]);
  assert!(!env_defers(&base));
  for key in [
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_CEILING_DIRECTORIES",
    "GIT_CONFIG_COUNT",
  ] {
    assert!(env_defers(&base.clone().with(key, "x")), "{key}");
  }
  assert!(
    !env_defers(&base.clone().with("GIT_DIR", "")),
    "empty is unset"
  );
  let sudo = base.with("SUDO_UID", "1000");
  assert_eq!(env_defers(&sudo), nix::unistd::geteuid().is_root());
}

#[test]
fn core_worktree_bare_and_includes_move_the_worktree() {
  let moves = |text: &str| moves_worktree(text, true);
  assert!(!moves(
    "[core]\n\trepositoryformatversion = 0\n\tbare = false\n"
  ));
  assert!(moves("[core]\n\tworktree = /elsewhere\n"));
  assert!(moves("[Core]\n\tBare = true ; comment\n"));
  assert!(
    moves("[core] bare\n"),
    "a key on the header line, no value, is true"
  );
  assert!(!moves("[core]\n\tbare =\n"), "an empty value is false");
  assert!(!moves("[remote \"core\"]\n\tworktree = x\n"));
  assert!(!moves("[core \"sub\"]\n\tworktree = x\n"));
  assert!(moves("[includeIf \"gitdir:/x\"]\n\tpath = y\n"));
  assert!(moves("[include]\n\tpath = y\n"));
  assert!(!moves("[unterminated\n\tworktree = x\n"));
  assert!(
    !moves_worktree("[core]\n\tbare = true\n", false),
    "bare repositories say so"
  );
  assert!(moves_worktree("[core]\n\tworktree = /w\n", false));
}

#[test]
fn git_booleans_follow_git() {
  for yes in ["true", "yes", "on", "1", "\"true\"", "TRUE # c"] {
    assert!(truthy(yes), "{yes}");
  }
  for no in ["false", "no", "off", "0", "", " ; c"] {
    assert!(!truthy(no), "{no}");
  }
}

#[test]
fn a_config_worktree_file_or_config_text_defers() {
  let dir = tempfile::tempdir().unwrap();
  let git = dir.path();
  assert!(!config_defers(git, git, true), "no config at all");
  std::fs::write(git.join("config"), "[core]\n\tbare = false\n").unwrap();
  assert!(!config_defers(git, git, true));
  std::fs::write(git.join("config"), "[core]\n\tworktree = ../w\n").unwrap();
  assert!(config_defers(git, git, false));
  std::fs::write(git.join("config"), "").unwrap();
  std::fs::write(git.join("config.worktree"), "").unwrap();
  assert!(config_defers(git, git, true));
}

#[test]
fn a_path_owned_by_another_user_or_missing_is_foreign() {
  let dir = tempfile::tempdir().unwrap();
  let mine = dir.path().join("mine");
  std::fs::write(&mine, "").unwrap();
  assert!(!foreign_owner(&[mine.as_path()]));
  assert!(foreign_owner(&[dir.path().join("missing").as_path()]));
  if nix::unistd::geteuid().is_root() {
    std::os::unix::fs::chown(&mine, Some(65534), None).unwrap();
    assert!(foreign_owner(&[mine.as_path()]));
  } else {
    assert!(foreign_owner(&[Path::new("/")]));
  }
}
