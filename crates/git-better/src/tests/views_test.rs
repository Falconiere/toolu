//! How commits and the branch context read as text and JSON.

use serde_json::json;

use super::{area_text, areas_line, commit_json, commit_text, context_json, context_text};
use crate::areas::Area;
use crate::context::{Context, Dirty};
use crate::history::Commit;

fn area(name: &str, added: u64) -> Area {
  Area {
    name: name.to_owned(),
    files: 1,
    added,
    deleted: 0,
  }
}

fn feature() -> Commit {
  Commit {
    sha: "0123456789abcdef".to_owned(),
    subject: "feat(cli): add x".to_owned(),
    kind: Some("feat".to_owned()),
    scope: Some("cli".to_owned()),
    intent: Some("agents need x".to_owned()),
    risk: Some("medium".to_owned()),
    refs: vec!["#1".to_owned()],
    breaking: None,
    areas: vec![area("crates/cli", 4)],
    branch: Some("feat/1-x".to_owned()),
  }
}

fn context(on_base: bool) -> Context {
  Context {
    branch: "feat/653-pause-it".to_owned(),
    base: "main".to_owned(),
    range: "refs/heads/main..HEAD".to_owned(),
    on_base,
    total: 2,
    commits: vec![
      Commit {
        subject: "wip: half".to_owned(),
        ..Commit::default()
      },
      feature(),
    ],
    dirty: Dirty {
      staged: 1,
      unstaged: 0,
      untracked: 2,
    },
  }
}

#[test]
fn areas_read_largest_first_and_cap_at_eight() {
  assert_eq!(area_text(&area("docs", 3)), "docs (+3 -0, 1 file)");
  let many: Vec<Area> = (0..10)
    .map(|index| area(&format!("a{index}"), index))
    .collect();
  let line = areas_line(&many);
  assert!(line.starts_with("a9 (+9 -0, 1 file), a8"), "{line}");
  assert!(line.ends_with(", 2 more"), "{line}");
}

#[test]
fn a_commit_reads_why_where_and_risk() {
  assert_eq!(
    commit_text(&feature()),
    "01234567 feat(cli): add x\n  why: agents need x\n  where: crates/cli (+4 -0, 1 file)\n  risk: medium"
  );
  let bare = Commit {
    sha: "abc".to_owned(),
    subject: "fix stuff".to_owned(),
    ..Commit::default()
  };
  assert_eq!(
    commit_text(&bare),
    "abc fix stuff\n  why: (no Intent: trailer)"
  );
  assert_eq!(commit_json(&feature())["semantic"], true);
  assert_eq!(commit_json(&bare)["type"], json!(null));
}

#[test]
fn the_context_reads_the_branch_its_commits_and_the_tree() {
  assert_eq!(
    context_text(&context(false)),
    "Branch feat/653-pause-it (feat, ref 653: pause it): 2 commits ahead of main; 1 semantic.\n\
     Areas: crates/cli (+4 -0, 1 file)\n\
     Highest risk: medium\n\
     Refs: #1\n\
     Why, newest first:\n\
     - feat(cli): add x: agents need x\n\
     Working tree: 1 staged, 0 unstaged, 2 untracked."
  );
  assert_eq!(
    context_text(&context(true)),
    "Branch feat/653-pause-it (feat, ref 653: pause it): the base branch.\n\
     Working tree: 1 staged, 0 unstaged, 2 untracked."
  );
  let document = context_json(&context(false));
  assert_eq!(document["verb"], "context");
  assert_eq!(document["risk"], "medium");
  assert_eq!(document["dirty"]["untracked"], 2);
  assert_eq!(document["commits"].as_array().map(Vec::len), Some(2));
}
