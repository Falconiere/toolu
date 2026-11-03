use super::local::local_git;
use super::{Target, export, import, verify_heads};
use toolu_runtime::env::Env;
use toolu_runtime::process::{Spec, run};

fn git(repo: &str, args: &[&str]) -> String {
  local_git(&Env::process(), repo, args).unwrap()
}

#[test]
fn validates_target_and_bundle_identity() {
  assert!(Target::new(None, None).unwrap().is_none());
  assert!(
    Target::new(Some("host"), Some("/srv/repo"))
      .unwrap()
      .is_some()
  );
  for (host, path) in [
    (Some("host"), None),
    (Some("../host"), Some("/repo")),
    (Some("host"), Some("/a/../b")),
    (Some("host"), Some("relative")),
  ] {
    assert!(Target::new(host, path).is_err());
  }
  assert!(
    verify_heads(
      "abc refs/toolu-transfer/run/snapshot",
      "abc",
      "refs/toolu-transfer/run/snapshot"
    )
    .is_ok()
  );
  assert!(
    verify_heads(
      "wrong refs/toolu-transfer/run/snapshot",
      "abc",
      "refs/toolu-transfer/run/snapshot"
    )
    .is_err()
  );
}

#[test]
fn real_git_bundle_round_trip_keeps_worktree_and_rejects_wrong_identity() {
  let source = tempfile::tempdir().unwrap();
  let destination = tempfile::tempdir().unwrap();
  let src = source.path().to_str().unwrap();
  let dst = destination.path().to_str().unwrap();
  for repo in [src, dst] {
    git(repo, &["init", "-q"]);
    git(repo, &["config", "user.name", "Test"]);
    git(repo, &["config", "user.email", "test@example.invalid"]);
  }
  std::fs::write(source.path().join("file"), b"committed\0bytes").unwrap();
  git(src, &["add", "file"]);
  git(src, &["commit", "-qm", "snapshot"]);
  let sha = git(src, &["rev-parse", "HEAD"]);
  std::fs::write(source.path().join("file"), b"uncommitted").unwrap();
  let bundle = export(&Env::process(), src, "run1", &sha).unwrap();
  assert!(import(&Env::process(), dst, "run2", &sha, &bundle).is_err());
  import(&Env::process(), dst, "run1", &sha, &bundle).unwrap();
  assert_eq!(
    git(dst, &["rev-parse", "refs/toolu-transfer/run1/snapshot"]),
    sha
  );
  let output = run(&Spec::new([
    "git",
    "-C",
    dst,
    "show",
    "refs/toolu-transfer/run1/snapshot:file",
  ]))
  .unwrap();
  assert_eq!(output.stdout_bytes, b"committed\0bytes");
  assert!(export(&Env::process(), src, "run1", &sha).is_err());
}

#[test]
fn failed_commands_carry_the_tail_of_their_stderr() {
  let failed = super::command(
    &Env::process(),
    &["sh", "-c", "echo 'fatal: could not read' >&2; exit 128"],
    &[],
  )
  .unwrap_err();
  assert_eq!(failed.code, "command_failed");
  assert_eq!(failed.message, "command exited 128: fatal: could not read");
  assert_eq!(super::failure_message(3, ""), "command exited 3");
  let long = "é".repeat(500);
  let message = super::failure_message(1, &long);
  assert!(message.starts_with("command exited 1: é"));
  assert!(message.len() <= "command exited 1: ".len() + super::STDERR_TAIL);
}
