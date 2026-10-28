//! The branch context from real repositories: the base branch, a feature
//! branch against a local or remote base, the working tree, and no repository.

use super::{Dirty, gather};
use crate::git_support::{Repo, git};
use crate::settings::Settings;

#[test]
fn on_the_base_branch_the_range_is_head() {
  let repo = Repo::new(".claude", None);
  let found = gather(&repo.env, &repo.root, &Settings::default(), None, 5).unwrap();
  assert!(found.on_base);
  assert_eq!(
    (found.branch.as_str(), found.range.as_str()),
    ("main", "HEAD")
  );
  assert_eq!(found.commits.len(), 1);
}

#[test]
fn a_feature_branch_reads_its_commits_and_the_working_tree() {
  let repo = Repo::new(".claude", None);
  git(&repo.root, &["switch", "-q", "feat/1-x"]);
  repo.commit("a.txt", "docs(a): add\n\nIntent: why", None);
  std::fs::write(repo.root.join("a.txt"), "changed\n").unwrap();
  std::fs::write(repo.root.join("b.txt"), "staged\n").unwrap();
  git(&repo.root, &["add", "b.txt"]);
  std::fs::write(repo.root.join("c.txt"), "untracked\n").unwrap();
  let found = gather(&repo.env, &repo.root, &Settings::default(), None, 5).unwrap();
  assert!(!found.on_base);
  assert_eq!(found.range, "refs/heads/main..HEAD");
  assert_eq!(found.commits.len(), 1);
  assert_eq!(
    found.dirty,
    Dirty {
      staged: 1,
      unstaged: 1,
      untracked: 1
    }
  );
  let given = gather(&repo.env, &repo.root, &Settings::default(), Some("HEAD"), 5).unwrap();
  assert_eq!(given.commits.len(), 2);
}

#[test]
fn a_base_only_on_origin_is_used_and_no_repository_is_an_error() {
  let repo = Repo::new(".claude", None);
  git(
    &repo.root,
    &["update-ref", "refs/remotes/origin/trunk", "main"],
  );
  git(
    &repo.root,
    &[
      "symbolic-ref",
      "refs/remotes/origin/HEAD",
      "refs/remotes/origin/trunk",
    ],
  );
  git(&repo.root, &["switch", "-q", "feat/1-x"]);
  let found = gather(&repo.env, &repo.root, &Settings::default(), None, 5).unwrap();
  assert_eq!(
    (found.base.as_str(), found.range.as_str()),
    ("trunk", "refs/remotes/origin/trunk..HEAD")
  );
  let empty = tempfile::tempdir().unwrap();
  let error = gather(&repo.env, empty.path(), &Settings::default(), None, 5).unwrap_err();
  assert!(error.ends_with("is not in a git repository"), "{error}");
}
