use super::{commit_denied, deny, must, push_blocked, quality, quality_state, ran_in};

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
fn deny_needs_an_unchanged_env_file_and_deny_evidence() {
  let dir = tempfile::tempdir().unwrap();
  std::fs::write(dir.path().join(".env"), "KEEP=1\n").unwrap();
  let seen = r#"{"hook":"PreToolUse","reason":"protected file .env"}"#;
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
fn deny_without_an_env_file_is_an_error() {
  let dir = tempfile::tempdir().unwrap();
  assert!(
    deny(dir.path(), "PreToolUse .env")
      .unwrap_err()
      .contains("cannot read")
  );
}

#[test]
fn ran_in_accepts_the_given_and_the_resolved_path() {
  let dir = tempfile::tempdir().unwrap();
  let given = dir.path().display().to_string();
  assert!(ran_in(dir.path(), &format!("{{\"output\":\"{given}\\n\"}}")).is_ok());
  let real = std::fs::canonicalize(dir.path())
    .unwrap()
    .display()
    .to_string();
  assert!(ran_in(dir.path(), &real).is_ok());
  assert!(ran_in(dir.path(), "/somewhere/else").is_err());
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
