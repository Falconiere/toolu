use super::{CWD_MARKER, commit_denied, deny, must, push_blocked, quality, quality_state, ran_in};
use crate::host_smoke_prompts::{CLAUDE_ALLOWED, CODEX_ALLOWED};

const FAILING: &str =
  "{\n  \"status\": \"failing\",\n  \"errors\": [\"TS file exceeds 3-line limit\"]\n}";
fn eight() -> String {
  "export const a = 1;\n".repeat(8)
}

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
fn ran_in_needs_the_marker_before_the_given_or_resolved_path() {
  let dir = tempfile::tempdir().unwrap();
  let given = dir.path().display().to_string();
  let printed = format!("{{\"output\":\"{CWD_MARKER}{given}\\n\"}}");
  assert!(ran_in(dir.path(), &printed).is_ok());
  let real = std::fs::canonicalize(dir.path())
    .unwrap()
    .display()
    .to_string();
  assert!(ran_in(dir.path(), &format!("{CWD_MARKER}{real}")).is_ok());
  assert!(ran_in(dir.path(), &format!("{CWD_MARKER}/somewhere/else")).is_err());
}

#[test]
fn the_working_directory_alone_is_not_evidence_the_control_ran() {
  let dir = tempfile::tempdir().unwrap();
  let init = format!(
    "{{\"type\":\"system\",\"subtype\":\"init\",\"cwd\":\"{}\"}}",
    dir.path().display()
  );
  assert_eq!(
    ran_in(dir.path(), &init).unwrap_err(),
    "the clean control command did not run"
  );
}

#[test]
fn both_allowed_prompts_print_the_marker_with_the_shell_working_directory() {
  for prompt in [CLAUDE_ALLOWED, CODEX_ALLOWED] {
    assert!(prompt.contains(&format!("{CWD_MARKER}$PWD")), "{prompt}");
  }
}

#[test]
fn quality_state_needs_the_failure_and_the_edit() {
  assert!(quality_state(FAILING, &eight()).is_ok());
  let passing = "{\"status\": \"passing\"}";
  assert_eq!(
    quality_state(passing, &eight()).unwrap_err(),
    "quality failure not recorded"
  );
  assert_eq!(
    quality_state(FAILING, "export const n = 1;\n").unwrap_err(),
    "source edit missing"
  );
}

#[test]
fn quality_reads_the_host_directory() {
  let dir = tempfile::tempdir().unwrap();
  assert!(
    quality(dir.path(), ".codex")
      .unwrap_err()
      .contains("quality gate file missing")
  );
  std::fs::create_dir_all(dir.path().join(".codex/tmp")).unwrap();
  std::fs::write(
    dir.path().join(".codex/tmp/quality-gate-status.json"),
    FAILING,
  )
  .unwrap();
  std::fs::write(dir.path().join("sample.ts"), eight()).unwrap();
  assert!(quality(dir.path(), ".codex").is_ok());
}

#[test]
fn a_commit_is_denied_only_by_the_quality_gate() {
  assert!(commit_denied("BLOCKED: quality gate failing - fix the violations").is_ok());
  assert!(commit_denied("[main abc] test: quality gate").is_err());
}

#[test]
fn a_push_is_blocked_by_either_review_reason() {
  let block = "Code review required before push (diff SHA abc, base main).";
  let ask = "No clean review is recorded for this diff. Approving pushes anyway";
  assert!(push_blocked(block).is_ok());
  assert!(push_blocked(ask).is_ok());
  assert!(push_blocked("BLOCKED: quality gate failing").is_err());
  assert!(push_blocked("To /tmp/remote.git * [new branch] HEAD -> feat/smoke").is_err());
}
