use toolu_runtime::env::Env;

use super::{Git, pin_head, preconditions};
use crate::rebase::Target;
use crate::support::{git, repo_with_origin};

fn env() -> Env {
  Env::from_pairs([(
    "PATH",
    "/usr/bin:/bin:/usr/local/bin:/opt/homebrew/bin".to_owned(),
  )])
}

#[test]
fn a_failed_command_reports_git_s_own_words() {
  let tmp = tempfile::tempdir().expect("temp");
  let env = env();
  let runner = Git::new(tmp.path(), &env);
  let err = runner
    .out(&["rev-parse", "HEAD"])
    .expect_err("not a repository");
  assert!(err.contains("not a git repository"), "{err}");
}

#[test]
fn a_rebase_in_progress_is_never_touched_and_the_head_can_be_pinned() {
  let tmp = tempfile::tempdir().expect("temp");
  let work = tmp.path().join("w");
  repo_with_origin(&work, "/nowhere");
  git(&work, &["checkout", "-qb", "feat/x"]);
  let head = git(&work, &["rev-parse", "HEAD"]);
  let target = Target {
    worktree: work.clone(),
    key: "k".to_owned(),
    branch: "feat/x".to_owned(),
    base: "main".to_owned(),
    head: head.clone(),
  };
  let env = env();
  let runner = Git::new(&work, &env);
  assert_eq!(preconditions(&runner, &target), Ok(()));
  std::fs::create_dir_all(work.join(".git/rebase-merge")).expect("marker");
  assert_eq!(
    preconditions(&runner, &target),
    Err("a rebase-merge is in progress".to_owned())
  );
  assert_eq!(pin_head(&runner, "k", &head), Ok(()));
  assert_eq!(git(&work, &["rev-parse", "refs/epic-wip/k"]), head);
  let err = pin_head(&runner, "k", "not-a-sha").expect_err("bad sha");
  assert!(err.starts_with("snapshot failed: "), "{err}");
}
