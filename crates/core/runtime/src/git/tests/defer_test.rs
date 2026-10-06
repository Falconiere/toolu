use std::path::Path;

use super::{Core, core_config, env_defers, foreign_owner, parse_core, truthy};
use crate::env::Env;

#[test]
fn redirecting_variables_defer_even_when_empty() {
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
    env_defers(&base.clone().with("GIT_DIR", "")),
    "git treats an empty GIT_DIR as set"
  );
  let sudo = base.with("SUDO_UID", "1000");
  assert_eq!(env_defers(&sudo), nix::unistd::geteuid().is_root());
}

fn core(bare: bool, worktree: Option<&str>) -> Core {
  Core {
    bare,
    worktree: worktree.map(str::to_owned),
  }
}

#[test]
fn core_bare_and_worktree_are_read_like_git() {
  let cases = [
    (
      "[core]\n\trepositoryformatversion = 0\n\tbare = false\n",
      false,
      None,
    ),
    (
      "[core]\n\tworktree = ../../../sub # c\n",
      false,
      Some("../../../sub"),
    ),
    ("[Core]\n\tBare = true ; comment\n", true, None),
    ("[core] bare\n", true, None),
    ("[core]\n\tbare =\n", false, None),
    ("[core]\n\tbare = yes\n[core]\n\tbare = off\n", false, None),
    ("[remote \"core\"]\n\tworktree = x\n", false, None),
    ("[core \"sub\"]\n\tworktree = x\n", false, None),
    ("[core]\n\trepositoryformatversion = 1\n", false, None),
  ];
  for (text, bare, worktree) in cases {
    assert_eq!(parse_core(text), Some(core(bare, worktree)), "{text:?}");
  }
}

#[test]
fn what_only_git_can_read_defers() {
  for text in [
    "[includeIf \"gitdir:/x\"]\n\tpath = y\n",
    "[include]\n\tpath = y\n",
    "[unterminated\n",
    "[core]\n\tworktree = \"/a b\"\n",
    "[core]\n\tworktree = a\\\\b\n",
    "[core]\n\tworktree\n",
    "[core]\n\tworktree =\n",
    "[core]\n\trepositoryformatversion = 2\n",
  ] {
    assert_eq!(parse_core(text), None, "{text:?}");
  }
}

#[test]
fn git_booleans_follow_git() {
  for yes in ["true", "yes", "on", "1", "TRUE"] {
    assert!(truthy(yes), "{yes}");
  }
  for no in ["false", "no", "off", "0", "", "False"] {
    assert!(!truthy(no), "{no}");
  }
}

#[test]
fn a_config_worktree_file_defers_and_a_missing_config_is_empty() {
  let dir = tempfile::tempdir().unwrap();
  let git = dir.path();
  assert_eq!(core_config(git, git), Some(core(false, None)), "no config");
  std::fs::write(git.join("config"), "[core]\n\tbare = true\n").unwrap();
  assert_eq!(core_config(git, git), Some(core(true, None)));
  std::fs::write(git.join("config.worktree"), "").unwrap();
  assert_eq!(core_config(git, git), None);
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
