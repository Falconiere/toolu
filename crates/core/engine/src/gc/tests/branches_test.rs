use super::super::repos::records;
use super::sweep;
use crate::gc::support::{Repo, git};

#[test]
fn merged_branches_go_and_checked_out_or_unmerged_ones_stay() {
  let repo = Repo::new().unwrap();
  git(&repo.env, &repo.root, &["branch", "merged-a"]).unwrap();
  git(&repo.env, &repo.root, &["branch", "ahead"]).unwrap();
  git(&repo.env, &repo.root, &["checkout", "-q", "ahead"]).unwrap();
  repo.commit(&repo.root, "ahead-file").unwrap();
  git(&repo.env, &repo.root, &["checkout", "-q", "main"]).unwrap();
  let checked = repo.tmp.join(".herdr/worktrees/repo/busy");
  repo.worktree(&checked, "merged-busy").unwrap();

  let report = sweep(&repo.request(false), &repo.root, &[]);
  assert_eq!(report["removed"], serde_json::json!(["merged-a"]));
  assert_eq!(report["kept"][0]["name"], "merged-busy");
  assert_eq!(report["kept"][0]["reason"], "checked out in a worktree");

  let applied = sweep(&repo.request(true), &repo.root, &[]);
  assert_eq!(applied["removed"], serde_json::json!(["merged-a"]));
  assert_eq!(applied["kept"][0]["name"], "merged-busy");
  let left = git(
    &repo.env,
    &repo.root,
    &["branch", "--format=%(refname:short)"],
  )
  .unwrap();
  assert!(left.contains("ahead") && left.contains("merged-busy") && !left.contains("merged-a"));
}

#[test]
fn without_all_branches_only_the_records_branches_are_in_scope() {
  let repo = Repo::new().unwrap();
  git(&repo.env, &repo.root, &["branch", "release/1.2"]).unwrap();
  git(&repo.env, &repo.root, &["branch", "feat/k1"]).unwrap();
  repo.record("k1", "merged", &[]).unwrap();
  let found = records(&[repo.tmp.join("epic")]);
  let mut request = repo.request(true);
  request.all_branches = false;
  let report = sweep(&request, &repo.root, &found);
  assert_eq!(report["removed"], serde_json::json!(["feat/k1"]));
  let left = git(
    &repo.env,
    &repo.root,
    &["branch", "--format=%(refname:short)"],
  )
  .unwrap();
  assert!(
    left.contains("release/1.2") && !left.contains("feat/k1"),
    "{left}"
  );
}
