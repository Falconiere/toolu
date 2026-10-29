use super::super::record::Record;
use super::{backend, git, local_worktree};
use serde_json::json;
use toolu_mux::Backend;
use toolu_runtime::env::Env;

#[test]
fn saved_backend_wins_and_legacy_records_keep_their_interpretation() {
  let env = Env::from_pairs([("TOOLU_EPIC_MULTIPLEXER", "herdr-app")]);
  let mut record = Record::new();
  assert_eq!(backend(&record, &env).unwrap(), Backend::HerdrApp);
  record.insert("kind".into(), json!("claude"));
  assert_eq!(backend(&record, &env).unwrap(), Backend::Herdr);
  record.insert("pane_id".into(), json!("%7"));
  assert_eq!(backend(&record, &env).unwrap(), Backend::Tmux);
  record.insert("backend".into(), json!("herdr-app"));
  assert_eq!(backend(&record, &env).unwrap(), Backend::HerdrApp);
  record.insert("backend".into(), json!("garbage"));
  assert!(backend(&record, &env).is_err());
}

#[test]
fn git_worktrees_reuse_only_the_expected_branch() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path().join("repo with spaces");
  std::fs::create_dir(&root).unwrap();
  let cwd = root.to_str().unwrap();
  let env = Env::process();
  git(&env, cwd, &["init", "--initial-branch=main"]).unwrap();
  git(
    &env,
    cwd,
    &[
      "-c",
      "user.name=Test",
      "-c",
      "user.email=test@example.invalid",
      "commit",
      "--allow-empty",
      "-m",
      "base",
    ],
  )
  .unwrap();
  let wt = dir.path().join("worker with spaces");
  local_worktree(&env, cwd, "feat/test", "main", wt.to_str().unwrap()).unwrap();
  local_worktree(&env, cwd, "feat/test", "main", wt.to_str().unwrap()).unwrap();
  assert!(local_worktree(&env, cwd, "feat/test", "main", cwd).is_err());
}

#[test]
fn a_local_delivery_worker_keeps_remote_cleanup_metadata_without_remote_dispatch() {
  let mut record = Record::new();
  record.insert("machine".into(), json!("vps"));
  record.insert("worktree".into(), json!("/local/mirror"));
  record.insert("remote_worktree".into(), json!("/remote/worker"));
  assert!(super::remote_worker(&record));
  assert_eq!(super::worker_cwd(&record), "/remote/worker");
  record.insert("controller_review".into(), json!(true));
  assert!(!super::remote_worker(&record));
  assert_eq!(super::worker_cwd(&record), "/local/mirror");
}
