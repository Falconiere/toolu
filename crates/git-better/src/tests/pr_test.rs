//! The PR draft: a title from one commit or from the branch, a body of intents
//! and areas, and a squash message whose trailers merge every commit's.

use super::draft;
use crate::areas::Area;
use crate::context::{Context, Dirty};
use crate::history::Commit;

fn commit(subject: &str, intent: Option<&str>, risk: Option<&str>) -> Commit {
  let parsed = crate::format::subject(subject);
  Commit {
    sha: "0".repeat(40),
    subject: subject.to_owned(),
    kind: parsed.map(|found| found.kind.to_owned()),
    scope: parsed.and_then(|found| found.scope).map(str::to_owned),
    intent: intent.map(str::to_owned),
    risk: risk.map(str::to_owned),
    areas: vec![Area {
      name: "crates/epic".to_owned(),
      files: 1,
      added: 2,
      deleted: 1,
    }],
    ..Commit::default()
  }
}

fn context(branch: &str, newest_first: Vec<Commit>) -> Context {
  let total = newest_first.len();
  Context {
    branch: branch.to_owned(),
    base: "main".to_owned(),
    range: "refs/heads/main..HEAD".to_owned(),
    on_base: false,
    total,
    commits: newest_first,
    dirty: Dirty::default(),
  }
}

#[test]
fn one_semantic_commit_is_the_title() {
  let only = context(
    "my-branch",
    vec![commit(
      "fix(epic): retry timeouts",
      Some("timeouts lost work"),
      None,
    )],
  );
  let found = draft(&only);
  assert_eq!(found.title, "fix(epic): retry timeouts");
  assert_eq!(
    found.body,
    "## Summary\n\n- timeouts lost work (`fix(epic): retry timeouts`)\n\n## Areas\n\n- crates/epic (+2 -1, 1 file)\n\n## Risk\n\nnot rated"
  );
  assert_eq!(
    found.squash,
    "fix(epic): retry timeouts\n\n- fix(epic): retry timeouts\n\nIntent: timeouts lost work"
  );
}

#[test]
fn many_commits_title_from_the_branch_and_merge_their_trailers() {
  let mut breaking = commit(
    "feat(epic)!: drop the old queue",
    Some("one queue"),
    Some("high"),
  );
  breaking.breaking = Some("the old queue is gone".to_owned());
  breaking.refs = vec!["#653".to_owned()];
  let mut first = commit(
    "feat(epic): add pause",
    Some("workers can wait"),
    Some("low"),
  );
  first.refs = vec!["ENG-1".to_owned()];
  let commits = vec![
    commit("fixup! feat(epic): add pause", None, None),
    breaking,
    commit("wip: half", None, None),
    first,
  ];
  let found = draft(&context("feat/653-pause-and-resume", commits));
  assert_eq!(found.title, "feat(epic)!: pause and resume");
  assert!(found.body.starts_with(
    "## Summary\n\n- workers can wait (`feat(epic): add pause`)\n- one queue (`feat(epic)!: drop the old queue`)\n"
  ));
  assert!(found.body.contains("\n## Risk\n\nhigh\n"), "{}", found.body);
  assert!(
    found
      .body
      .contains("\n## Breaking changes\n\n- the old queue is gone\n")
  );
  assert!(found.body.ends_with("\n\nCloses #653"), "{}", found.body);
  assert_eq!(
    found.squash,
    "feat(epic)!: pause and resume\n\n- feat(epic): add pause\n- feat(epic)!: drop the old queue\n\n\
     Intent: workers can wait; one queue\nRisk: high\nRefs: #653, ENG-1\n\
     BREAKING-CHANGE: the old queue is gone"
  );
}

#[test]
fn without_semantic_commits_the_draft_leaves_placeholders() {
  let found = draft(&context(
    "my-branch",
    vec![commit("fixed stuff", None, None)],
  ));
  assert_eq!(found.title, "<type>(<area>): <what changed>");
  assert!(
    found
      .squash
      .ends_with("Intent: <why this change is needed>")
  );
}

#[test]
fn a_draft_from_part_of_the_branch_says_so() {
  let mut partial = context(
    "feat/1-x",
    vec![commit("feat(x): add y", Some("y is needed"), None)],
  );
  partial.total = 30;
  let found = draft(&partial);
  assert!(
    found
      .body
      .ends_with("\n\n_Drafted from the newest 1 of 30 commits._"),
    "{}",
    found.body
  );
}
