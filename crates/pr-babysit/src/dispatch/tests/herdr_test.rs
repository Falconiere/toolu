use std::os::unix::fs::PermissionsExt;
use std::time::Duration;

use toolu_runtime::env::Env;

use super::{accept_claude_trust, argv, command, herdr, reachable, stop_agent, try_herdr};

const SECOND: Duration = Duration::from_secs(1);

fn with_herdr(dir: &std::path::Path, reply: &str) -> Env {
  let bin = dir.join("bin");
  std::fs::create_dir_all(&bin).unwrap();
  let script = bin.join("herdr");
  std::fs::write(&script, format!("#!/bin/sh\ncat <<'EOF'\n{reply}\nEOF\n")).unwrap();
  std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
  Env::from_pairs([("PATH", format!("{}:/usr/bin:/bin", bin.display()))])
}

#[test]
fn without_herdr_nothing_is_reachable_and_no_agent_is_live() {
  let env = Env::from_pairs([("PATH", "/nonexistent")]);
  assert!(!reachable(&env));
  assert!(stop_agent(&env, "pb-1"));
  assert!(!accept_claude_trust(&env, "pb-1", "/w"));
  let error = try_herdr(&env, &argv(&["agent", "list"]), SECOND)
    .err()
    .unwrap();
  assert_eq!(error.code, "invalid_output");
  let failure = herdr(&env, &argv(&["worktree", "create"])).err().unwrap();
  assert_eq!(failure.code, "herdr_error");
  assert!(
    failure
      .message
      .starts_with("herdr worktree create: invalid_output: exit 1")
  );
  assert_eq!(command(&env, &argv(&["git", "status"]), SECOND).status, 1);
}

#[test]
fn herdr_results_and_errors_are_unwrapped() {
  let dir = tempfile::tempdir().unwrap();
  let env = with_herdr(
    dir.path(),
    r#"{"id":"1","result":{"workspace":{"workspace_id":"w1"}}}"#,
  );
  let opened = herdr(&env, &argv(&["worktree", "open"])).unwrap();
  assert_eq!(opened["workspace"]["workspace_id"], "w1");
  assert!(reachable(&env));
  let env = with_herdr(
    dir.path(),
    r#"{"error":{"code":"agent_not_found","message":"no agent pb-1"}}"#,
  );
  let error = try_herdr(&env, &argv(&["agent", "get", "pb-1"]), SECOND)
    .err()
    .unwrap();
  assert_eq!(
    (error.code.as_str(), error.message.as_str()),
    ("agent_not_found", "no agent pb-1")
  );
  let failure = herdr(&env, &argv(&["agent", "get", "pb-1"])).err().unwrap();
  assert_eq!(
    failure.message,
    "herdr agent get: agent_not_found: no agent pb-1"
  );
  assert_eq!(failure.extra["herdrCode"], "agent_not_found");
}
