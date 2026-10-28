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

#[test]
fn a_short_renamed_path_is_counted_once_and_the_total_ignores_the_limit() {
  let repo = Repo::new(".claude", None);
  repo.commit("a", "docs(a): a", None);
  git(&repo.root, &["switch", "-q", "feat/1-x"]);
  git(&repo.root, &["merge", "-q", "--ff-only", "main"]);
  git(&repo.root, &["mv", "a", "b"]);
  std::fs::write(repo.root.join("zz"), "z\n").unwrap();
  git(&repo.root, &["add", "zz"]);
  let dirty = gather(&repo.env, &repo.root, &Settings::default(), None, 5)
    .unwrap()
    .dirty;
  assert_eq!(
    dirty,
    Dirty {
      staged: 2,
      unstaged: 0,
      untracked: 0
    }
  );
  git(&repo.root, &["commit", "-q", "-m", "docs(a): move"]);
  repo.commit("c", "docs(c): c", None);
  let limited = gather(&repo.env, &repo.root, &Settings::default(), None, 1).unwrap();
  assert_eq!((limited.commits.len(), limited.total), (1, 2));
}

#[test]
fn an_unborn_branch_is_named_and_a_detached_head_is_head() {
  let dir = tempfile::tempdir().unwrap();
  let repo = Repo::new(".claude", None);
  git(dir.path(), &["init", "-q", "-b", "feat/9-new"]);
  let unborn = gather(&repo.env, dir.path(), &Settings::default(), None, 5).unwrap();
  assert_eq!((unborn.branch.as_str(), unborn.total), ("feat/9-new", 0));
  assert_eq!(unborn.commits, []);
  git(&repo.root, &["switch", "-q", "feat/1-x"]);
  repo.commit("d.txt", "docs(d): d", None);
  git(&repo.root, &["switch", "-q", "--detach", "HEAD"]);
  let detached = gather(&repo.env, &repo.root, &Settings::default(), None, 5).unwrap();
  assert_eq!(detached.branch, "HEAD");
  assert_eq!(
    (detached.range.as_str(), detached.total),
    ("refs/heads/main..HEAD", 1)
  );
}
