use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::time::Duration;

use super::tests::{Repo, env, repo, resources, stage, take};
use super::{Lease, Rebased, git::Git, rebase, replay};
use crate::support::git;
use toolu_engine::resources::leases::LeaseKind;

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
  // The hook has to start before the client deadline, and still be asleep when it fires.
  hook(
    &repo.origin.join("hooks/post-receive"),
    &format!("#!/bin/sh\n{break_inspection}sleep 45\n"),
  );
  let runtime_env = env(&repo);
  let target = repo.target();
  let runner = Git::with_timeout(&repo.work, &runtime_env, Duration::from_secs(20));
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

#[test]
fn a_pre_push_hook_refusal_hands_off_without_a_remote_write() {
  let repo = repo(("b.txt", "feature\n"), ("a.txt", "ONE\ntwo\n"));
  hook(
    &repo.work.join(".git/hooks/pre-push"),
    "#!/bin/sh\nprintf 'blocked by pre-push\\n' >&2\nexit 1\n",
  );
  let result = rebase(&repo.target(), None, &env(&repo), None);
  let Rebased::Handoff(reason) = result else {
    panic!("{result:?}");
  };
  assert!(reason.contains("pre-push"), "{reason}");
  assert_eq!(repo.remote("feat/x"), repo.target().head);
}

#[test]
fn a_pre_push_hook_refusal_hands_off_even_if_remote_inspection_fails() {
  let repo = repo(("b.txt", "feature\n"), ("a.txt", "ONE\ntwo\n"));
  let moved = repo.tmp.path().join("origin.git.gone");
  hook(
    &repo.work.join(".git/hooks/pre-push"),
    &format!(
      "#!/bin/sh\nmv '{}' '{}'\nprintf 'blocked by pre-push\\n' >&2\nexit 1\n",
      repo.origin.display(),
      moved.display()
    ),
  );
  let result = rebase(&repo.target(), None, &env(&repo), None);
  let Rebased::Handoff(reason) = result else {
    panic!("{result:?}");
  };
  assert!(reason.contains("pre-push"), "{reason}");
  assert_eq!(git(&moved, &["rev-parse", "feat/x"]), repo.target().head);
}

#[test]
fn an_unfence_failure_keeps_the_reason_and_can_be_recovered() {
  let repo = repo(("b.txt", "feature\n"), ("a.txt", "ONE\ntwo\n"));
  let root = resources(&repo);
  let agent = take(&root, LeaseKind::Agent, &repo.work);
  let lease = Lease {
    root: root.clone(),
    token: agent.clone(),
  };
  super::set_stage(&lease, "rebasing").expect("fence");
  let state = root.join("state.json");
  let saved = fs::read(&state).expect("state");
  hook(
    &repo.work.join(".git/hooks/pre-push"),
    &format!("#!/bin/sh\nprintf '{{' > '{}'\nexit 1\n", state.display()),
  );
  let outcome = rebase(&repo.target(), Some(&lease), &env(&repo), None);
  let Rebased::Handoff(reason) = outcome else {
    panic!("{outcome:?}");
  };
  assert!(reason.contains("unfence failed:"), "{reason}");
  assert!(reason.contains("recovery failed:"), "{reason}");
  fs::write(state, saved).expect("restore state");
  super::recover(Some(&repo.work), Some(&lease), &env(&repo)).expect("recover");
  assert_eq!(stage(&root, &agent), "running");
  assert_eq!(repo.remote("feat/x"), repo.target().head);
}
