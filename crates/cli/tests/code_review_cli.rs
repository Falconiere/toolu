//! `toolu code-review`, black-box: a bad effort is a usage error, a repository
//! explicitly disabled gets `not-enabled`, and an enabled one with no key
//! fails closed before reading the diff. The help text is pinned by
//! the generated `docs/cli/code-review.md`, which `cargo xtask docs-cli --check`
//! compares.

use std::path::Path;
use std::process::{Command, Output};

fn toolu(args: &[&str], project: Option<&Path>) -> std::io::Result<Output> {
  let mut command = Command::new(env!("CARGO_BIN_EXE_toolu"));
  if let Some(project) = project {
    // The user config comes from the project root, which holds none, never from $HOME.
    command
      .env("TOOLU_PROJECT_DIR", project)
      .env("TOOLU_CONFIG_DIR", project)
      .env_remove("OPENROUTER_API_KEY")
      .env_remove("TOOLU_USER_CONFIG_DIR");
  }
  command.args(args).output()
}

/// A project directory whose `.claude/toolu.config.json` holds `config`.
fn project(config: &str) -> std::io::Result<tempfile::TempDir> {
  let dir = tempfile::tempdir()?;
  std::fs::create_dir_all(dir.path().join(".claude"))?;
  std::fs::write(dir.path().join(".claude/toolu.config.json"), config)?;
  Ok(dir)
}

#[test]
fn an_unknown_effort_exits_64_with_nothing_on_stdout() {
  let output = toolu(&["code-review", "run", "--effort", "huge"], None).unwrap();
  assert_eq!(output.status.code(), Some(64));
  assert!(output.stdout.is_empty(), "{:?}", output.stdout);
}

#[test]
fn explicitly_disabled_review_exits_69_and_explains_configuration() {
  let off = project(r#"{"codeReview":{"enabled":false}}"#).unwrap();
  let output = toolu(
    &["--host", "claude", "code-review", "run"],
    Some(off.path()),
  )
  .unwrap();
  assert_eq!(output.status.code(), Some(69));
  assert!(output.stderr.is_empty(), "{:?}", output.stderr);
  let stderr = String::from_utf8(output.stdout).unwrap();
  assert!(
    stderr.contains("not-enabled: code review is disabled"),
    "{stderr}"
  );
  assert!(stderr.contains("project toolu.config.json"), "{stderr}");
}

#[test]
fn a_default_enabled_run_without_a_key_fails_closed_and_uses_the_configured_effort() {
  let on = project(r#"{"codeReview":{"effort":"small"}}"#).unwrap();
  let output = toolu(
    &["--json", "--host", "claude", "code-review", "run"],
    Some(on.path()),
  )
  .unwrap();
  assert_eq!(output.status.code(), Some(69));
  let doc: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
  assert_eq!(doc["error"]["kind"], "unconfigured");
  assert_eq!(doc["effort"], "small");
  let message = doc["error"]["message"].as_str().unwrap();
  assert!(
    message.contains("OPENROUTER_API_KEY is missing"),
    "{message}"
  );
}

#[test]
fn config_dir_supplies_the_user_level_effort() {
  let on = project(r#"{"codeReview":{"enabled":true}}"#).unwrap();
  let user = tempfile::tempdir().unwrap();
  std::fs::write(
    user.path().join("toolu.config.json"),
    r#"{"codeReview":{"effort":"xhigh"}}"#,
  )
  .unwrap();
  let dir = user.path().to_str().unwrap();
  let args = [
    "--json",
    "--host",
    "claude",
    "--config-dir",
    dir,
    "code-review",
    "run",
  ];
  let output = toolu(&args, Some(on.path())).unwrap();
  let doc: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
  assert_eq!(doc["effort"], "xhigh");
}

#[test]
fn absent_configuration_is_enabled_and_invalid_configuration_reports_why() {
  let absent = tempfile::tempdir().unwrap();
  let output = toolu(
    &[
      "--json",
      "--host",
      "claude",
      "code-review",
      "run",
      "--effort",
      "xhigh",
    ],
    Some(absent.path()),
  )
  .unwrap();
  assert_eq!(output.status.code(), Some(69));
  let doc: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
  assert_eq!(doc["error"]["kind"], "unconfigured");
  assert_eq!(doc["effort"], "xhigh");
  for config in ["{", r#"{"codeReview":{"enabled":"yes"}}"#, r#"{"nope":1}"#] {
    let invalid = project(config).unwrap();
    let output = toolu(
      &["--json", "--host", "claude", "code-review", "run"],
      Some(invalid.path()),
    )
    .unwrap();
    assert_eq!(output.status.code(), Some(69));
    let doc: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(doc["error"]["kind"], "not-enabled");
    assert_ne!(doc["notes"], serde_json::json!([]));
    assert_eq!(doc["findings"], serde_json::json!([]));
  }
}

/// The committed golden set.
fn cases() -> std::path::PathBuf {
  Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/code-review-eval")
}

#[test]
fn eval_without_live_needs_no_key_and_a_replay_miss_exits_69_unverified() {
  // Disabled and keyless: a replay run must not care about either.
  let off = project(r#"{"codeReview":{"enabled":false}}"#).unwrap();
  let empty = tempfile::tempdir().unwrap();
  let cases = cases();
  let args = [
    "--json",
    "--host",
    "claude",
    "code-review",
    "eval",
    "--cases",
    cases.to_str().unwrap(),
    "--replay-dir",
    empty.path().to_str().unwrap(),
    "--effort",
    "small",
    "--arm",
    "baseline",
  ];
  let output = toolu(&args, Some(off.path())).unwrap();
  assert_eq!(output.status.code(), Some(69), "{output:?}");
  let doc: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
  let suite = doc["cases"].as_array().unwrap();
  assert_eq!(suite.len(), 7);
  for case in suite {
    assert_eq!(case["arms"].as_array().unwrap().len(), 0);
    let why = case["unverified"][0].as_str().unwrap();
    assert!(why.contains("replay miss: review-"), "{why}");
  }
  assert!(String::from_utf8_lossy(&output.stderr).contains("unverified"));
  // Nothing was recorded either: replay never writes.
  assert_eq!(std::fs::read_dir(empty.path()).unwrap().count(), 0);
}

#[test]
fn eval_live_without_a_key_exits_69_before_anything_is_sent_or_recorded() {
  let on = project("{}").unwrap();
  let store = tempfile::tempdir().unwrap();
  let record = store.path().join("replay");
  let cases = cases();
  let args = [
    "--host",
    "claude",
    "code-review",
    "eval",
    "--cases",
    cases.to_str().unwrap(),
    "--live",
    "--replay-dir",
    record.to_str().unwrap(),
  ];
  let output = toolu(&args, Some(on.path())).unwrap();
  assert_eq!(output.status.code(), Some(69), "{output:?}");
  assert!(output.stdout.is_empty(), "{:?}", output.stdout);
  let stderr = String::from_utf8(output.stderr).unwrap();
  assert!(stderr.contains("OPENROUTER_API_KEY is missing"), "{stderr}");
  assert!(stderr.contains("unverified"), "{stderr}");
  assert!(!record.exists());
}

#[test]
fn eval_live_disabled_or_invalid_configuration_is_unverified_without_recordings() {
  for (config, reason) in [
    (
      r#"{"codeReview":{"enabled":false}}"#,
      "disabled by configuration",
    ),
    (r#"{"codeReview":{"enabled":"yes"}}"#, "must be a boolean"),
    ("{", "malformed JSON"),
  ] {
    let off = project(config).unwrap();
    let store = tempfile::tempdir().unwrap();
    let record = store.path().join("replay");
    let cases = cases();
    let output = toolu(
      &[
        "--host",
        "claude",
        "code-review",
        "eval",
        "--cases",
        cases.to_str().unwrap(),
        "--live",
        "--replay-dir",
        record.to_str().unwrap(),
      ],
      Some(off.path()),
    )
    .unwrap();
    assert_eq!(output.status.code(), Some(69));
    assert_eq!(output.stdout, [] as [u8; 0]);
    let message = String::from_utf8(output.stderr).unwrap();
    assert!(message.contains(reason), "{message}");
    assert!(message.contains("unverified"), "{message}");
    assert!(!record.exists());
  }
}

#[test]
fn eval_usage_errors_exit_64() {
  let cases = cases();
  let cases = cases.to_str().unwrap();
  let missing = tempfile::tempdir().unwrap();
  let attempts: [&[&str]; 4] = [
    &["code-review", "eval"],
    &["code-review", "eval", "--cases", cases, "--effort", "huge"],
    &["code-review", "eval", "--cases", cases, "--arm", "third"],
    &[
      "code-review",
      "eval",
      "--cases",
      missing.path().to_str().unwrap(),
    ],
  ];
  for args in attempts {
    let output = toolu(args, None).unwrap();
    assert_eq!(output.status.code(), Some(64), "{args:?}: {output:?}");
    assert!(output.stdout.is_empty(), "{args:?}");
  }
}

#[test]
fn eval_help_lists_the_arms_the_levels_with_none_and_the_replay_flags() {
  let output = toolu(&["code-review", "eval", "--help"], None).unwrap();
  assert_eq!(output.status.code(), Some(0));
  let help = String::from_utf8(output.stdout).unwrap();
  for needle in [
    "--cases",
    "--arm",
    "--effort",
    "--live",
    "--replay-dir",
    "--report",
    "baseline",
    "enhanced",
    "none",
    "xhigh",
  ] {
    assert!(help.contains(needle), "{needle} missing from {help}");
  }
  // `none` is an eval-only baseline; `run` still refuses it.
  let output = toolu(&["code-review", "run", "--effort", "none"], None).unwrap();
  assert_eq!(output.status.code(), Some(64));
}
