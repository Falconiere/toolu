//! A branch's commits read from real git: subject parts, trailers, areas from
//! `--numstat`, the provenance note's branch, the limit, and the summaries.

use super::{Commit, highest_risk, history, union};
use crate::areas::{AREA_ROOTS, Area};
use crate::git_support::{Repo, git};
use crate::note::{Provenance, write};

fn roots() -> Vec<String> {
  AREA_ROOTS.iter().map(|root| (*root).to_owned()).collect()
}

/// `feat/1-x` with a semantic feature, a `wip` and a breaking fix.
fn branch() -> Repo {
  let repo = Repo::new(".claude", None);
  git(&repo.root, &["switch", "-q", "feat/1-x"]);
  repo.commit(
    "crates/cli/src/a.rs",
    "feat(cli): add x\n\nIntent: agents need x\nRisk: low\nRefs: #1, ENG-2",
    None,
  );
  repo.commit("docs/notes.md", "wip: half", None);
  repo.commit(
    "crates/core/b.rs",
    "fix(core)!: drop y\n\nIntent: y was wrong\nRisk: high\nBREAKING-CHANGE: y is gone",
    None,
  );
  repo
}

#[test]
fn each_commit_carries_its_subject_trailers_and_areas() {
  let repo = branch();
  let found = history(&repo.env, &repo.root, "main..HEAD", 20, &roots()).unwrap();
  let subjects: Vec<&str> = found.iter().map(|commit| commit.subject.as_str()).collect();
  assert_eq!(
    subjects,
    ["fix(core)!: drop y", "wip: half", "feat(cli): add x"]
  );
  let feature = &found[2];
  assert_eq!(
    (feature.kind.as_deref(), feature.scope.as_deref()),
    (Some("feat"), Some("cli"))
  );
  assert_eq!(feature.intent.as_deref(), Some("agents need x"));
  assert_eq!(feature.refs, ["#1", "ENG-2"]);
  assert_eq!(
    feature.areas,
    [Area {
      name: "crates/cli".to_owned(),
      files: 1,
      added: 5,
      deleted: 0
    }]
  );
  assert!(feature.semantic() && !found[1].semantic());
  assert_eq!(found[0].breaking.as_deref(), Some("y is gone"));
  assert_eq!(found[0].branch, None);
}

#[test]
fn the_note_names_the_branch_and_the_limit_caps_the_range() {
  let repo = branch();
  let provenance = Provenance {
    branch: "feat/1-x".to_owned(),
    base: "main".to_owned(),
    host: "claude".to_owned(),
    session: "s".to_owned(),
  };
  write(&repo.env, &repo.root, "HEAD", &provenance, &roots()).unwrap();
  let found = history(&repo.env, &repo.root, "main..HEAD", 1, &roots()).unwrap();
  assert_eq!(found.len(), 1);
  assert_eq!(found[0].branch.as_deref(), Some("feat/1-x"));
  assert!(history(&repo.env, &repo.root, "nope..HEAD", 5, &roots()).is_err());
}

#[test]
fn areas_sum_and_the_highest_risk_wins() {
  let repo = branch();
  let found = history(&repo.env, &repo.root, "main..HEAD", 20, &roots()).unwrap();
  let names: Vec<String> = union(&found).into_iter().map(|area| area.name).collect();
  assert_eq!(names, ["crates/cli", "crates/core", "docs"]);
  assert_eq!(highest_risk(&found), Some("high"));
  assert_eq!(highest_risk(&found[1..]), Some("low"));
  assert_eq!(highest_risk(&[Commit::default()]), None);
}

#[test]
fn a_dash_range_is_refused_before_git_sees_it() {
  let repo = branch();
  let target = repo.root.join("written");
  let range = format!("--output={}", target.display());
  let error = history(&repo.env, &repo.root, &range, 5, &roots()).unwrap_err();
  assert_eq!(error, format!("`{range}` is not a revision range"));
  assert!(!target.exists());
}

#[test]
fn separator_bytes_in_a_message_make_no_phantom_commit() {
  let repo = branch();
  repo.commit("x.txt", "docs(x): odd\n\nIntent: a\u{1e}b\u{1f}c", None);
  let found = history(&repo.env, &repo.root, "main..HEAD", 20, &roots()).unwrap();
  assert_eq!(found.len(), 4);
  assert_eq!(found[0].intent.as_deref(), Some("a\u{1e}b\u{1f}c"));
}

#[test]
fn a_merge_reads_its_changes_against_the_first_parent() {
  let repo = branch();
  git(&repo.root, &["switch", "-q", "-c", "fix/2-y", "main"]);
  repo.commit("crates/other/c.rs", "fix(other): c\n\nIntent: c", None);
  git(&repo.root, &["switch", "-q", "feat/1-x"]);
  git(&repo.root, &["merge", "-q", "--no-edit", "fix/2-y"]);
  let found = history(&repo.env, &repo.root, "main..HEAD", 1, &roots()).unwrap();
  let names: Vec<&str> = found[0]
    .areas
    .iter()
    .map(|area| area.name.as_str())
    .collect();
  assert_eq!(names, ["crates/other"]);
}
