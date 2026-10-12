use std::path::{Path, PathBuf};
use std::time::SystemTime;

use toolu_engine::resources::leases::{LeaseKind, LeaseRequest, acquire_lease};
use toolu_engine::resources::store::{leases, read_resource_state};
use toolu_runtime::env::Env;
use toolu_runtime::json::ordered::Ordered;

use super::{Lease, Rebased, Target, rebase};
use crate::support::{git, origin_with_clone};

/// A bare origin, the worker's clone on `feat/x` (pushed), and `main` moved
/// on by another clone.
struct Repo {
  tmp: tempfile::TempDir,
  work: PathBuf,
  origin: PathBuf,
  head: String,
}

const BASE: [(&str, &str); 2] = [("a.txt", "one\ntwo\n"), ("b.txt", "b\n")];

fn repo(branch_edit: (&str, &str), main_edit: (&str, &str)) -> Repo {
  let tmp = tempfile::tempdir().expect("temp");
  let origin = tmp.path().join("origin.git");
  let work = tmp.path().join("work");
  origin_with_clone(&origin, &work, &BASE);
  git(&work, &["config", "user.name", "worker"]);
  git(&work, &["config", "user.email", "worker@localhost"]);
  git(&work, &["checkout", "-qb", "feat/x"]);
  std::fs::write(work.join(branch_edit.0), branch_edit.1).expect("edit");
  git(&work, &["commit", "-qam", "feature"]);
  git(&work, &["push", "-q", "-u", "origin", "feat/x"]);
  let other = tmp.path().join("other");
  git(tmp.path(), &["clone", "-q", &origin.display().to_string(), "other"]);
  std::fs::write(other.join(main_edit.0), main_edit.1).expect("main edit");
  git(&other, &["commit", "-qam", "main moves"]);
  git(&other, &["push", "-q", "origin", "main"]);
  let head = git(&work, &["rev-parse", "HEAD"]);
  Repo {
    tmp,
    work,
    origin,
    head,
  }
}

impl Repo {
  fn target(&self) -> Target {
    Target {
      worktree: self.work.clone(),
      key: "k".to_owned(),
      branch: "feat/x".to_owned(),
      base: "main".to_owned(),
      head: self.head.clone(),
    }
  }

  fn remote(&self, branch: &str) -> String {
    git(&self.origin, &["rev-parse", branch])
  }

  fn bytes(&self) -> Vec<(String, Vec<u8>)> {
    ["a.txt", "b.txt", "u.txt"]
      .iter()
      .map(|name| (name.to_string(), std::fs::read(self.work.join(name)).unwrap_or_default()))
      .collect()
  }
}

fn env(repo: &Repo) -> Env {
  Env::from_pairs([
    ("PATH", "/usr/bin:/bin:/usr/local/bin:/opt/homebrew/bin".to_owned()),
    ("HOME", repo.tmp.path().display().to_string()),
  ])
}

#[test]
fn a_clean_branch_is_rebased_pushed_with_lease_and_snapshotted_first() {
  let repo = repo(("b.txt", "feature\n"), ("a.txt", "ONE\ntwo\n"));
  let outcome = rebase(&repo.target(), None, &env(&repo), None);
  let Rebased::Pushed(head) = outcome else {
    panic!("{outcome:?}");
  };
  assert_ne!(head, repo.head);
  assert_eq!(repo.remote("feat/x"), head, "the PR branch moved");
  let main = repo.remote("main");
  git(&repo.work, &["merge-base", "--is-ancestor", &main, &head]);
  assert_eq!(
    git(&repo.work, &["rev-parse", "refs/epic-wip/k"]),
    repo.head,
    "the snapshot names the pre-rebase head"
  );
}

#[test]
fn a_conflict_aborts_and_leaves_the_worktree_byte_identical_to_its_snapshot() {
  let repo = repo(("a.txt", "uno\ntwo\n"), ("a.txt", "ONE\ntwo\n"));
  std::fs::write(repo.work.join("u.txt"), "untracked work\n").expect("untracked");
  let before = repo.bytes();
  let reflog = git(&repo.work, &["reflog", "--format=%H"]);
  let outcome = rebase(&repo.target(), None, &env(&repo), None);
  let Rebased::Handoff(reason) = outcome else {
    panic!("{outcome:?}");
  };
  assert!(reason.starts_with("conflict: "), "{reason}");
  assert!(!reason.contains("unverified"), "{reason}");
  assert_eq!(repo.bytes(), before);
  assert_eq!(git(&repo.work, &["rev-parse", "HEAD"]), repo.head);
  assert_eq!(git(&repo.work, &["status", "--porcelain"]), "?? u.txt");
  assert_eq!(repo.remote("feat/x"), repo.head, "nothing was pushed");
  let snapshot = git(&repo.work, &["show", "refs/epic-wip/k:u.txt"]);
  assert_eq!(snapshot, "untracked work");
  assert_eq!(git(&repo.work, &["show", "refs/epic-wip/k:a.txt"]), "uno\ntwo");
  let after = git(&repo.work, &["reflog", "--format=%H"]);
  assert!(after.ends_with(&reflog), "the abort only appends to the reflog");
}

#[test]
fn unsafe_worktrees_are_handed_to_the_worker_without_a_git_write() {
  let repo = repo(("b.txt", "feature\n"), ("a.txt", "ONE\ntwo\n"));
  std::fs::write(repo.work.join("a.txt"), "edited\n").expect("dirty");
  let mut cases = vec![(repo.target(), "uncommitted changes to tracked files")];
  let mut other = repo.target();
  "feat/y".clone_into(&mut other.branch);
  cases.push((other, "worktree is on feat/x, not feat/y"));
  let mut stale = repo.target();
  "0000000000000000000000000000000000000000".clone_into(&mut stale.head);
  cases.push((stale, "is not the PR head"));
  let mut base = repo.target();
  "feat/x".clone_into(&mut base.base);
  cases.push((base, "refusing to rebase branch"));
  for (target, expected) in cases {
    let outcome = rebase(&target, None, &env(&repo), None);
    let Rebased::Handoff(reason) = outcome else {
      panic!("{outcome:?}");
    };
    assert!(reason.contains(expected), "{reason}");
  }
  assert_eq!(git(&repo.work, &["rev-parse", "HEAD"]), repo.head);
  assert!(
    std::process::Command::new("git")
      .args(["-C", &repo.work.display().to_string(), "rev-parse", "--verify", "-q", "refs/epic-wip/k"])
      .output()
      .expect("git")
      .stdout
      .is_empty(),
    "no snapshot was taken"
  );
}

fn resources(repo: &Repo) -> PathBuf {
  let root = repo.tmp.path().join("resources");
  std::fs::create_dir_all(&root).expect("root");
  std::fs::write(
    root.join("policy.json"),
    r#"{"maxAgents": 3, "maxJobs": 2, "hosts": {"claude": 1}}"#,
  )
  .expect("policy");
  root
}

fn take(root: &Path, kind: LeaseKind, worktree: &Path) -> String {
  let request = LeaseRequest {
    kind,
    key: "k".into(),
    state_dir: "epic".into(),
    host: (kind == LeaseKind::Agent).then(|| "claude".to_owned()),
    host_cap: None,
    epic_cap: None,
    worktree: Some(worktree.display().to_string()),
  };
  match acquire_lease(root, &request, SystemTime::now())
    .expect("lease")
    .get("token")
  {
    Some(Ordered::String(token)) => token.clone(),
    other => panic!("{other:?}"),
  }
}

fn stage(root: &Path, token: &str) -> String {
  let state = read_resource_state(root).expect("state");
  leases(&state)
    .iter()
    .find(|lease| matches!(lease.get("token"), Some(Ordered::String(t)) if t == token))
    .and_then(|lease| match lease.get("stage") {
      Some(Ordered::String(stage)) => Some(stage.clone()),
      _ => None,
    })
    .unwrap_or_default()
}

#[test]
fn a_live_job_in_the_worktree_keeps_the_engine_out_and_the_fence_is_lifted() {
  let repo = repo(("b.txt", "feature\n"), ("a.txt", "ONE\ntwo\n"));
  let root = resources(&repo);
  let agent = take(&root, LeaseKind::Agent, &repo.work);
  let lease = Lease {
    root: root.clone(),
    token: agent.clone(),
  };
  let _job = take(&root, LeaseKind::Job, &repo.work);
  let outcome = rebase(&repo.target(), Some(&lease), &env(&repo), None);
  assert_eq!(outcome, Rebased::Busy("a managed job runs in the worktree".to_owned()));
  assert_eq!(stage(&root, &agent), "running");
  assert_eq!(git(&repo.work, &["rev-parse", "HEAD"]), repo.head);
}

#[test]
fn a_fenced_rebase_runs_and_restores_the_agent_stage() {
  let repo = repo(("b.txt", "feature\n"), ("a.txt", "ONE\ntwo\n"));
  let root = resources(&repo);
  let agent = take(&root, LeaseKind::Agent, &repo.work);
  let lease = Lease {
    root: root.clone(),
    token: agent.clone(),
  };
  let lost = Lease {
    root: root.clone(),
    token: "gone".to_owned(),
  };
  let outcome = rebase(&repo.target(), Some(&lost), &env(&repo), None);
  assert_eq!(outcome, Rebased::Busy("resource lease lost".to_owned()));
  let outcome = rebase(&repo.target(), Some(&lease), &env(&repo), None);
  assert!(matches!(outcome, Rebased::Pushed(_)), "{outcome:?}");
  assert_eq!(stage(&root, &agent), "running");
}
