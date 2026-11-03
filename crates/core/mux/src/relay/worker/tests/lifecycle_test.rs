//! Real HC and Git checks for result export, report reads and cleanup.

use super::super::preparation_tests::{Fixture, command};
use super::super::{reported, result};
use super::cleanup;
use std::path::Path;

/// Commit a `result` file in the attempt worktree and return the verified
/// result commit; the dirty tree is refused first.
fn commit_result(fixture: &Fixture) -> String {
  std::fs::write(Path::new(&fixture.tree).join("result"), "fixed\n").unwrap();
  assert!(result(&fixture.env, &fixture.task, &fixture.worker, &fixture.tree).is_err());
  command(&fixture.env, &["git", "-C", &fixture.tree, "add", "result"]).unwrap();
  command(
    &fixture.env,
    &[
      "git",
      "-C",
      &fixture.tree,
      "-c",
      "user.name=Test",
      "-c",
      "user.email=test@example.invalid",
      "commit",
      "-qm",
      "result",
    ],
  )
  .unwrap();
  let (commit, bundle) =
    result(&fixture.env, &fixture.task, &fixture.worker, &fixture.tree).unwrap();
  assert_ne!(bundle, [] as [u8; 0]);
  commit
}

#[test]
fn committed_result_and_report_are_bound_to_the_attempt_and_base() {
  let fixture = Fixture::new().unwrap();
  fixture.setup().unwrap();
  let commit = commit_result(&fixture);
  let attempt = fixture.task.attempt.as_deref().unwrap();
  let path = Path::new(&fixture.worker.root)
    .join("tasks")
    .join(format!("{attempt}-report.json"));
  assert_eq!(
    reported(&fixture.env, &fixture.task, &fixture.worker, &fixture.tree).unwrap(),
    None
  );
  std::fs::write(&path, "{not json").unwrap();
  assert!(
    reported(&fixture.env, &fixture.task, &fixture.worker, &fixture.tree)
      .unwrap_err()
      .starts_with("unreadable worker report")
  );
  std::fs::write(
    &path,
    serde_json::json!({"attempt":"wrong", "base":fixture.task.request.base, "commit":commit})
      .to_string(),
  )
  .unwrap();
  assert_eq!(
    reported(&fixture.env, &fixture.task, &fixture.worker, &fixture.tree).unwrap(),
    None
  );
  std::fs::write(
    &path,
    serde_json::json!({"attempt":attempt, "base":fixture.task.request.base, "commit":commit})
      .to_string(),
  )
  .unwrap();
  assert_eq!(
    reported(&fixture.env, &fixture.task, &fixture.worker, &fixture.tree)
      .unwrap()
      .unwrap()["commit"],
    commit
  );
}

#[test]
fn cleanup_removes_the_settled_worktree_once_and_prunes_the_clone() {
  let fixture = Fixture::new().unwrap();
  fixture.setup().unwrap();
  commit_result(&fixture);
  let attempt = fixture.task.attempt.as_deref().unwrap();
  cleanup(&fixture.env, &fixture.worker, attempt, &fixture.tree).unwrap();
  assert!(!Path::new(&fixture.tree).exists());
  assert_eq!(
    command(
      &fixture.env,
      &[
        "git",
        "-C",
        &fixture.repo,
        "worktree",
        "list",
        "--porcelain"
      ]
    )
    .unwrap()
    .matches("worktree ")
    .count(),
    1
  );
  cleanup(&fixture.env, &fixture.worker, attempt, &fixture.tree).unwrap();
}
