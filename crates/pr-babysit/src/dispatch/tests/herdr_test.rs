use std::os::unix::fs::PermissionsExt;
use std::time::Duration;

use toolu_runtime::env::Env;

use super::{accept_claude_trust, argv, command, herdr, reachable, stop_agent, try_herdr};
use crate::fixer::host::Fixer;

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
  assert!(stop_agent(&env, "pb-1", None));
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
fn a_herdr_error_written_to_stderr_keeps_its_code() {
  let dir = tempfile::tempdir().unwrap();
  let env = with_herdr(dir.path(), "");
  let script = dir.path().join("bin/herdr");
  let body = r#"{"error":{"code":"agent_not_ready","message":"blocked during startup"}}"#;
  std::fs::write(&script, format!("#!/bin/sh\necho '{body}' >&2\nexit 1\n")).unwrap();
  let error = try_herdr(&env, &argv(&["agent", "start", "pb-1"]), SECOND)
    .err()
    .unwrap();
  assert_eq!(
    (error.code.as_str(), error.message.as_str()),
    ("agent_not_ready", "blocked during startup")
  );
  std::fs::write(&script, "#!/bin/sh\necho '{}' >&2\nexit 1\n").unwrap();
  let error = try_herdr(&env, &argv(&["agent", "list"]), SECOND)
    .err()
    .unwrap();
  assert_eq!(
    (error.code.as_str(), error.message.as_str()),
    ("invalid_output", "exit 1: {}")
  );
  std::fs::write(
    &script,
    "#!/bin/sh\necho 'server unreachable' >&2\nexit 1\n",
  )
  .unwrap();
  let error = try_herdr(&env, &argv(&["agent", "list"]), SECOND)
    .err()
    .unwrap();
  assert_eq!(error.message, "exit 1: server unreachable");
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

/// A herdr that logs each call and lists agent `pb-1` until the call named
/// `exits` arrives; `enter` answers `agent_not_found` once the agent is gone.
fn logging_herdr(dir: &std::path::Path, exits: &str) -> (Env, std::path::PathBuf) {
  let log = dir.join("calls.log");
  let gone = dir.join("gone");
  let script = format!(
    r#"#!/bin/sh
echo "$*" >> '{log}'
case "$*" in
  "agent list") if [ -e '{gone}' ]; then echo '{{"result":{{"agents":[]}}}}'; else echo '{{"result":{{"agents":[{{"name":"pb-1"}}]}}}}'; fi; exit 0 ;;
  "agent send-keys pb-1 enter") if [ -e '{gone}' ]; then echo '{{"error":{{"code":"agent_not_found","message":"agent target pb-1 not found"}}}}' >&2; exit 1; fi ;;
esac
case "$*" in
  "{exits}") touch '{gone}' ;;
esac
echo '{{"result":{{}}}}'
"#,
    log = log.display(),
    gone = gone.display(),
  );
  let env = with_herdr(dir, "");
  std::fs::write(dir.join("bin/herdr"), script).unwrap();
  (env, log)
}

fn calls(log: &std::path::Path) -> Vec<String> {
  let text = std::fs::read_to_string(log).unwrap();
  text
    .lines()
    .filter(|line| *line != "agent list")
    .map(str::to_owned)
    .collect()
}

#[test]
fn a_muse_fixer_is_stopped_with_enter_after_exit_and_other_hosts_are_not() {
  let exit = ["agent send-keys pb-1 esc", "agent prompt pb-1 /exit"];
  let enter = "agent send-keys pb-1 enter";
  for host in [Fixer::Claude, Fixer::Codex, Fixer::Cursor] {
    let dir = tempfile::tempdir().unwrap();
    let (env, log) = logging_herdr(dir.path(), "agent prompt pb-1 /exit");
    assert!(stop_agent(&env, "pb-1", Some(host)), "{host:?}");
    assert_eq!(calls(&log), exit, "{host:?}");
  }
  let busy = tempfile::tempdir().unwrap();
  let (env, log) = logging_herdr(busy.path(), enter);
  assert!(stop_agent(&env, "pb-1", Some(Fixer::Muse)));
  assert_eq!(calls(&log), [exit[0], exit[1], enter]);
  let idle = tempfile::tempdir().unwrap();
  let (env, log) = logging_herdr(idle.path(), "agent prompt pb-1 /exit");
  assert!(stop_agent(&env, "pb-1", Some(Fixer::Muse)));
  assert_eq!(calls(&log), [exit[0], exit[1], enter]);
}
