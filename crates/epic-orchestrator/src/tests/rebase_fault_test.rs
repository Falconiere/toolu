use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::time::Duration;

use super::tests::{Repo, env, repo};
use super::{Rebased, git::Git, rebase, replay};
use crate::support::git;

fn hook(path: &Path, script: &str) {
  fs::write(path, script).expect("hook");
  let mut permissions = fs::metadata(path).expect("metadata").permissions();
  permissions.set_mode(0o755);
  fs::set_permissions(path, permissions).expect("executable hook");
}

fn count_pushes(work: &Path) -> String {
  fs::read_to_string(work.join(".git/push-attempts")).expect("push attempts")
}

fn delayed_push(inspect_available: bool) -> (Repo, Rebased) {
  let repo = repo(("b.txt", "feature\n"), ("a.txt", "ONE\ntwo\n"));
  hook(
    &repo.work.join(".git/hooks/pre-push"),
    "#!/bin/sh\nprintf x >> .git/push-attempts\n",
  );
  let break_inspection = if inspect_available {
    String::new()
  } else {
    format!(
      "mv '{}' '{}.gone'\n",
      repo.origin.display(),
      repo.origin.display()
    )
  };
  hook(
    &repo.origin.join("hooks/post-receive"),
    &format!("#!/bin/sh\n{break_inspection}sleep 6\n"),
  );
  let runtime_env = env(&repo);
  let target = repo.target();
  let runner = Git::with_timeout(&repo.work, &runtime_env, Duration::from_secs(2));
  let result = replay(&runner, &target);
  (repo, result)
}

#[test]
fn a_lost_push_ack_is_recognized_by_remote_head_without_replay() {
  let (repo, result) = delayed_push(true);
  let Rebased::Pushed(head) = result else {
    panic!("{result:?}");
  };
  assert_eq!(repo.remote("feat/x"), head);
  assert_eq!(git(&repo.work, &["rev-parse", "HEAD"]), head);
  assert_eq!(count_pushes(&repo.work), "x");
}

#[test]
fn unavailable_remote_inspection_keeps_the_push_uncertain() {
  let (repo, result) = delayed_push(false);
  let Rebased::Uncertain(reason) = result else {
    panic!("{result:?}");
  };
  assert!(reason.contains("remote inspection failed"), "{reason}");
  assert_eq!(count_pushes(&repo.work), "x");
  let head = git(&repo.work, &["rev-parse", "HEAD"]);
  let moved = repo.tmp.path().join("origin.git.gone");
  assert_eq!(
    git(&moved, &["rev-parse", "feat/x"]),
    head,
    "the push did land"
  );
}

#[test]
fn a_real_lease_rejection_hands_off_without_overwriting_the_remote() {
  let repo = repo(("b.txt", "feature\n"), ("a.txt", "ONE\ntwo\n"));
  let other = repo.tmp.path().join("other");
  git(&other, &["checkout", "-qb", "feat/x", "origin/feat/x"]);
  fs::write(other.join("b.txt"), "rival\n").expect("rival edit");
  git(&other, &["commit", "-qam", "rival moves branch"]);
  git(&other, &["push", "-q", "origin", "feat/x"]);
  let rival = repo.remote("feat/x");
  let result = rebase(&repo.target(), None, &env(&repo), None);
  let Rebased::Handoff(reason) = result else {
    panic!("{result:?}");
  };
  assert!(reason.starts_with("push refused:"), "{reason}");
  assert!(reason.contains("[rejected]"), "{reason}");
  assert_eq!(repo.remote("feat/x"), rival);
  assert_ne!(git(&repo.work, &["rev-parse", "HEAD"]), rival);
}
