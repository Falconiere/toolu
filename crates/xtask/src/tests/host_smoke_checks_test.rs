use super::{deny, must, push_blocked, quality, quality_state, ran_in};
use crate::host_smoke_project::NONCE_FILE;
use crate::host_smoke_prompts::{CLAUDE_ALLOWED, CODEX_ALLOWED};

const FAILING: &str = "{\n  \"status\": \"failing\",\n  \"source\": \"gate-status-hook\",\n  \"reason\": \"Quality command failed: sh tools/smoke/test.sh (exit 1)\"\n}";

#[test]
fn must_reports_its_message() {
  assert!(must(true, "x").is_ok());
  assert_eq!(must(false, "x").unwrap_err(), "x");
}

#[test]
fn deny_needs_an_unchanged_env_file_and_the_protected_files_reason() {
  let dir = tempfile::tempdir().unwrap();
  std::fs::write(dir.path().join(".env"), "KEEP=1\n").unwrap();
  let seen = r#"{"reason":"This command would WRITE to .env, a protected path (matches \".env\"). Blocked by gates.protectedFiles.mode='block' (see gates.md)."}"#;
  assert!(deny(dir.path(), seen).is_ok());
  assert_eq!(
    deny(dir.path(), "nothing").unwrap_err(),
    "no live deny evidence"
  );
  std::fs::write(dir.path().join(".env"), "changed").unwrap();
  assert_eq!(
    deny(dir.path(), seen).unwrap_err(),
    "protected .env changed"
  );
}

#[test]
fn a_transcript_that_only_mentions_the_hook_and_the_file_is_not_a_deny() {
  let dir = tempfile::tempdir().unwrap();
  std::fs::write(dir.path().join(".env"), "KEEP=1\n").unwrap();
  let chatter = r#"{"hook":"PreToolUse","command":"printf changed > .env"}"#;
  assert_eq!(
    deny(dir.path(), chatter).unwrap_err(),
    "no live deny evidence"
  );
}

#[test]
fn deny_without_an_env_file_is_an_error() {
  let dir = tempfile::tempdir().unwrap();
  assert!(
    deny(dir.path(), "Blocked by gates.protectedFiles.mode='block'")
      .unwrap_err()
      .contains("cannot read")
  );
}

#[test]
fn ran_in_needs_the_nonce_in_the_transcript() {
  let printed = "{\"output\":\"1f2e3d-9a-77\\n\"}";
  assert!(ran_in("1f2e3d-9a-77", printed).is_ok());
  assert_eq!(
    ran_in("1f2e3d-9a-78", printed).unwrap_err(),
    "the clean control command did not run"
  );
  assert!(ran_in("", printed).is_err());
}

#[test]
fn the_working_directory_or_a_typed_marker_is_not_evidence_the_control_ran() {
  let init = "{\"type\":\"system\",\"subtype\":\"init\",\"cwd\":\"/tmp/p\"}";
  assert_eq!(
    ran_in("abc-12-34", init).unwrap_err(),
    "the clean control command did not run"
  );
  let typed = "{\"output\":\"toolu-smoke-cwd:/tmp/p\\n\"}";
  assert!(ran_in("abc-12-34", typed).is_err());
}

#[test]
fn both_allowed_prompts_cat_the_nonce_by_its_relative_path() {
  for prompt in [CLAUDE_ALLOWED, CODEX_ALLOWED] {
    assert!(prompt.contains(&format!("`cat {NONCE_FILE}`")), "{prompt}");
    assert!(!prompt.contains("echo"), "{prompt}");
  }
}

#[test]
fn quality_state_needs_the_gate_status_hook_failure_for_the_exact_command() {
  assert!(quality_state(FAILING).is_ok());
  for state in [
    "not json",
    "{\"status\": \"passing\", \"source\": \"gate-status-hook\", \"reason\": \"Quality command failed: sh tools/smoke/test.sh (exit 1)\"}",
    "{\"status\": \"failing\", \"source\": \"other-hook\", \"reason\": \"Quality command failed: sh tools/smoke/test.sh (exit 1)\"}",
    "{\"status\": \"failing\", \"source\": \"gate-status-hook\", \"reason\": \"Quality command failed: npm test (exit 1)\"}",
  ] {
    assert!(quality_state(state).is_err(), "{state}");
  }
}

#[test]
fn quality_reads_the_host_directory() {
  let dir = tempfile::tempdir().unwrap();
  assert!(
    quality(dir.path(), ".codex/tmp")
      .unwrap_err()
      .contains("quality gate file missing")
  );
  std::fs::create_dir_all(dir.path().join(".codex/tmp")).unwrap();
  std::fs::write(
    dir.path().join(".codex/tmp/quality-gate-status.json"),
    FAILING,
  )
  .unwrap();
  assert!(quality(dir.path(), ".codex/tmp").is_ok());
}

#[test]
fn a_push_is_blocked_by_either_review_reason() {
  let block = "Code review required before push (diff SHA abc, base main).";
  let ask = "No clean review is recorded for this diff. Approving pushes anyway";
  assert!(push_blocked(block).is_ok());
  assert!(push_blocked(ask).is_ok());
  assert!(push_blocked("BLOCKED: quality gate failing").is_ok());
  assert!(push_blocked("To /tmp/remote.git * [new branch] HEAD -> feat/smoke").is_err());
}
