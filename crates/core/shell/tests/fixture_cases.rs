//! The shared TypeScript fixtures (#416 AC-1): every case of
//! `fixtures/shell/bats-parity.json` (the inputs the six shipped bash functions
//! received) and `fixtures/shell/issue-283.json` (one fixture per #283 example)
//! gets from `toolu-shell` the answer `@toolu/core/shell` gives: the case's
//! `expected` when it has one, else its `bash` result. Push roots and branches
//! are resolved against real git repositories, as in
//! `packages/toolu-core/src/shell/__tests__/{issue-283,parity-helpers}.ts`.

#[path = "helpers/answer.rs"]
mod answer;
#[path = "helpers/cases.rs"]
mod cases;
#[path = "helpers/decide.rs"]
mod decide;
#[path = "helpers/git_repo.rs"]
mod git_repo;
#[path = "helpers/lists.rs"]
mod lists;

use cases::Res;

const BATS: &str = "fixtures/shell/bats-parity.json";
const ISSUE_283: &str = "fixtures/shell/issue-283.json";

const BATS_GIT: [&str; 2] = ["is_git_push", "is_git_commit"];
const ISSUE_283_GIT: [&str; 3] = ["is_git_push", "is_git_commit", "commit_messages"];
const ISSUE_283_STRUCTURE: [&str; 2] = ["commands", "exit_proves"];

/// How many cases a replay ran, and which of them the crate answered differently.
struct Report {
  rel: &'static str,
  replayed: usize,
  failures: Vec<String>,
}

impl Report {
  /// Fails with every mismatch unless exactly `count` cases ran and all matched.
  fn expect(self, count: usize) {
    let Report {
      rel,
      replayed,
      failures,
    } = self;
    assert!(
      failures.is_empty(),
      "{} of {replayed} {rel} cases differ:\n{}",
      failures.len(),
      failures.join("\n\n")
    );
    assert_eq!(replayed, count, "{rel} cases replayed");
  }
}

/// Replays every case of `rel` whose function is in `kinds`.
fn replay(rel: &'static str, kinds: &[&str]) -> Res<Report> {
  let mut replayed = 0;
  let mut failures = Vec::new();
  for case in cases::cases(rel)? {
    if !kinds.contains(&answer::kind(&case)?) {
      continue;
    }
    replayed += 1;
    failures.extend(answer::mismatch(&case)?);
  }
  Ok(Report {
    rel,
    replayed,
    failures,
  })
}

#[test]
fn bats_is_git_push_and_commit_match_bash() {
  replay(BATS, &BATS_GIT).unwrap().expect(102);
}

#[test]
fn bats_write_targets_match_bash_or_the_documented_difference() {
  replay(BATS, &["bash_write_targets"]).unwrap().expect(30);
}

#[test]
fn bats_commands_decide_matches_bash() {
  replay(BATS, &["bash_commands_decide"]).unwrap().expect(20);
}

#[test]
fn bats_push_target_root_matches_bash_on_real_repositories() {
  replay(BATS, &["push_target_root"]).unwrap().expect(11);
}

#[test]
fn bats_push_target_branch_matches_bash_attached_and_detached() {
  replay(BATS, &["push_target_branch"]).unwrap().expect(23);
}

#[test]
fn issue_283_git_questions_are_answered_correctly() {
  replay(ISSUE_283, &ISSUE_283_GIT).unwrap().expect(18);
}

#[test]
fn issue_283_write_targets_are_answered_correctly() {
  replay(ISSUE_283, &["bash_write_targets"])
    .unwrap()
    .expect(13);
}

#[test]
fn issue_283_commands_decide_is_answered_correctly() {
  replay(ISSUE_283, &["bash_commands_decide"])
    .unwrap()
    .expect(9);
}

#[test]
fn issue_283_push_target_root_is_answered_correctly_on_real_repositories() {
  replay(ISSUE_283, &["push_target_root"]).unwrap().expect(2);
}

#[test]
fn issue_283_commands_and_exit_status_are_answered_correctly() {
  replay(ISSUE_283, &ISSUE_283_STRUCTURE).unwrap().expect(6);
}

#[test]
fn issue_283_a_long_command_is_analyzed_within_its_budget() {
  replay(ISSUE_283, &["latency"]).unwrap().expect(1);
}

#[test]
fn the_tests_above_replay_every_case_of_both_fixtures() {
  let bats = [
    "is_git_push",
    "is_git_commit",
    "bash_write_targets",
    "bash_commands_decide",
    "push_target_root",
    "push_target_branch",
  ];
  let issue = [
    "is_git_push",
    "is_git_commit",
    "commit_messages",
    "bash_write_targets",
    "bash_commands_decide",
    "push_target_root",
    "commands",
    "exit_proves",
    "latency",
  ];
  replay(BATS, &bats).unwrap().expect(186);
  replay(ISSUE_283, &issue).unwrap().expect(49);
  assert_eq!(cases::cases(BATS).unwrap().len(), 186);
  assert_eq!(cases::cases(ISSUE_283).unwrap().len(), 49);
}
