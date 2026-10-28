//! The `SessionStart` summary: the format alone on the base branch, the branch
//! summary on a feature branch, and silence when off or outside a repository.

use super::{FORMAT, summary};
use crate::git_support::{Repo, git};

#[test]
fn the_base_branch_gets_the_format_alone() {
  let repo = Repo::new(".claude", None);
  assert_eq!(summary(&repo.env, &repo.root).as_deref(), Some(FORMAT));
}

#[test]
fn a_feature_branch_gets_its_summary() {
  let repo = Repo::new(".claude", None);
  git(&repo.root, &["switch", "-q", "feat/1-x"]);
  repo.commit("a.txt", "docs(a): add\n\nIntent: readers need a", None);
  let text = summary(&repo.env, &repo.root).unwrap();
  assert!(text.starts_with(FORMAT), "{text}");
  assert!(
    text.contains("\nBranch feat/1-x (feat, ref 1: x): 1 commit ahead of main; 1 semantic."),
    "{text}"
  );
  assert!(text.contains("- docs(a): add: readers need a"), "{text}");
}

#[test]
fn off_or_outside_a_repository_says_nothing() {
  let off = Repo::new(
    ".claude",
    Some(r#"{"version":1,"gitBetter":{"sessionContext":false}}"#),
  );
  assert_eq!(summary(&off.env, &off.root), None);
  let empty = tempfile::tempdir().unwrap();
  assert_eq!(summary(&off.env, empty.path()), None);
}

#[test]
fn a_branch_without_a_base_says_so_instead_of_claiming_to_be_it() {
  let repo = Repo::new(".claude", None);
  git(&repo.root, &["switch", "-q", "feat/1-x"]);
  git(&repo.root, &["branch", "-m", "main", "trunk"]);
  let text = summary(&repo.env, &repo.root).unwrap();
  assert_eq!(
    text,
    format!(
      "{FORMAT}\nBranch feat/1-x (feat, ref 1: x): the base branch main exists neither locally nor on origin, so the branch's own commits are unknown."
    )
  );
}
