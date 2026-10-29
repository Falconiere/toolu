use super::*;
use serde_json::json;

#[test]
fn mutation_requires_session_owner_and_worktree() {
  let env = Env::from_pairs([("HC_SESSION", "one")]);
  let record = json!({"hc_session":"one","lease_token":"lease", "terminal_owner":"toolu.owner=lease", "worktree":"/w"});
  let terminal = json!({"cwd":"/w", "labels":{"toolu.owner":"lease"}});
  assert!(matches(&record, &terminal, &env).is_ok());
  let other = Env::from_pairs([("HC_SESSION", "other")]);
  assert!(matches(&record, &terminal, &other).is_err());
  assert!(
    matches(
      &record,
      &json!({"cwd":"/w","labels":{"toolu.owner":"other"}}),
      &env
    )
    .is_err()
  );
  assert!(
    matches(
      &record,
      &json!({"cwd":"/other","labels":{"toolu.owner":"lease"}}),
      &env
    )
    .is_err()
  );
}

#[test]
fn unregistered_terminal_cannot_be_mutated() {
  let dir = tempfile::tempdir().unwrap();
  assert!(records(&Paths::at(dir.path()), "t1").is_err());
}

#[test]
fn registered_lookup_requires_a_unique_address() {
  let dir = tempfile::tempdir().unwrap();
  let state = dir.path().join("epic");
  let issues = state.join("issues");
  std::fs::create_dir_all(&issues).unwrap();
  let paths = Paths::at(dir.path());
  std::fs::write(
    paths.registry(),
    json!({"version":1,"epics":[{"state_dir":state}]}).to_string(),
  )
  .unwrap();
  let record =
    json!({"terminal_address":"t1","remote_terminal_address":"vm/t7","lease_token":"lease"});
  std::fs::write(issues.join("one.json"), record.to_string()).unwrap();
  assert_eq!(records(&paths, "t1").unwrap(), record);
  assert!(records(&paths, "vm/t7").is_err());
  std::fs::write(issues.join("two.json"), record.to_string()).unwrap();
  assert!(records(&paths, "t1").is_err());
  assert!(records(&paths, "t2").is_err());
}

#[test]
fn local_review_uses_primary_terminal_and_local_worktree() {
  let env = Env::from_pairs([("HC_SESSION", "one")]);
  let mut record = json!({"hc_session":"one", "lease_token":"lease", "terminal_owner":"toolu.owner=lease",
    "terminal_address":"t2", "remote_terminal_address":"vm/t1", "worktree":"/local", "remote_worktree":"/remote"});
  let local = json!({"cwd":"/local", "labels":{"toolu.owner":"lease"}});
  let remote = json!({"cwd":"/remote", "labels":{"toolu.owner":"lease"}});
  assert!(matches(&record, &remote, &env).is_ok());
  assert!(matches(&record, &local, &env).is_err());
  record["controller_review"] = json!(true);
  assert!(matches(&record, &local, &env).is_ok());
  assert!(matches(&record, &remote, &env).is_err());
}
