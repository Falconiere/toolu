use super::super::packages;
use super::{LEVELS, env, repo};
use crate::budget::Effort;
use crate::review::mask::Mask;

#[test]
fn local_snapshot_supplies_source_and_caller_context_after_worktree_changes() {
  let caller = "fn local_caller() { parse_level(\"local\"); }\n";
  let dir = repo(
    &[
      ("src/levels.rs", LEVELS.to_owned()),
      ("src/report.rs", caller.to_owned()),
    ],
    &[
      (
        "src/levels.rs",
        LEVELS.replace("\"warn\" => 3", "\"warn\" => 4"),
      ),
      ("src/report.rs", "fn committed_caller() {}\n".to_owned()),
    ],
  );
  let changed = LEVELS.replace("\"warn\" => 3", "\"warn\" => 7");
  std::fs::write(dir.path().join("src/levels.rs"), &changed).unwrap();
  std::fs::write(dir.path().join("src/report.rs"), caller).unwrap();
  let env = env();
  let state = dir.path().join(".codex/tmp");
  let shaped = crate::shape::review(&env, dir.path(), Some("main"), &state).unwrap();
  std::fs::write(dir.path().join("src/levels.rs"), "fn later() {}\n").unwrap();
  std::fs::write(dir.path().join("src/report.rs"), "fn later_caller() {}\n").unwrap();
  let (found, notes) = packages(
    &env,
    dir.path(),
    &shaped,
    Effort::High.budget(),
    &Mask::default(),
  );
  let package = found
    .iter()
    .find(|package| package.path == "src/levels.rs")
    .unwrap();
  assert_eq!(package.source.as_deref(), Some(changed.as_str()));
  let context = package.context.as_deref().unwrap_or_default();
  assert!(
    context.contains("fn local_caller() { parse_level(\"local\"); }"),
    "{context} {notes:?}"
  );
  assert!(!context.contains("later_caller"), "{context}");
}
