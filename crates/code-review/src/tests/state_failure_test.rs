use super::*;

#[test]
fn failed_usage_append_rotation_and_cleanup_replace_an_older_passing_gate_state() {
  let (repo, state, fixture) = clean_case();
  let env = env(repo.path(), state.path(), &fixture);
  assert_eq!(
    invoke(&env, &options(&fixture)).verdict(),
    Verdict::Approved
  );
  let log = repo.path().join(".codex/tmp/code-review-usage.jsonl");
  std::fs::remove_file(&log).unwrap();
  std::fs::create_dir(&log).unwrap();
  let report = invoke(&env, &options(&fixture));
  assert_eq!(
    report.error.as_ref().map(|error| error.kind),
    Some(ErrorKind::State)
  );
  assert_eq!(report.blocking_count(), 1);
  assert_eq!(saved_state(state.path())["findings_count"], 1);
  std::fs::remove_dir(&log).unwrap();
  std::fs::write(&log, "{}\n".repeat(350_000)).unwrap();
  let archive_dir = repo.path().join(".codex/tmp/code-review-usage");
  std::fs::write(&archive_dir, "occupied").unwrap();
  let report = invoke(&env, &options(&fixture));
  assert_eq!(
    report.error.as_ref().map(|error| error.kind),
    Some(ErrorKind::State)
  );
  assert_eq!(report.blocking_count(), 1);
  assert_eq!(saved_state(state.path())["findings_count"], 1);
  std::fs::remove_file(&archive_dir).unwrap();
  std::fs::write(&log, "{}\n").unwrap();
  std::fs::create_dir_all(archive_dir.join("00000000000000000001.jsonl")).unwrap();
  let report = invoke(&env, &options(&fixture));
  assert_eq!(
    report.error.as_ref().map(|error| error.kind),
    Some(ErrorKind::State)
  );
  assert_eq!(report.blocking_count(), 1);
  assert_eq!(saved_state(state.path())["findings_count"], 1);
  assert_eq!(std::fs::read_to_string(&log).unwrap().lines().count(), 2);
}
