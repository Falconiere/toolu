//! `toolu code-review`, black-box: a bad effort is a usage error, a repository
//! without the opt-in gets `not-enabled`, and an opted-in one still fails closed
//! until the last pipeline stages land (#668, #669). The help text is pinned by
//! the generated `docs/cli/code-review.md`, which `cargo xtask docs-cli --check`
//! compares.

use std::path::Path;
use std::process::{Command, Output};

fn toolu(args: &[&str], project: Option<&Path>) -> std::io::Result<Output> {
  let mut command = Command::new(env!("CARGO_BIN_EXE_toolu"));
  if let Some(project) = project {
    command.env("TOOLU_PROJECT_DIR", project);
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
fn run_without_the_opt_in_exits_69_and_says_how_to_enable() {
  let off = project(r#"{"codeReview":{"enabled":false}}"#).unwrap();
  let output = toolu(&["--host", "claude", "code-review", "run"], Some(off.path())).unwrap();
  assert_eq!(output.status.code(), Some(69));
  assert!(output.stdout.is_empty(), "{:?}", output.stdout);
  let stderr = String::from_utf8(output.stderr).unwrap();
  assert!(
    stderr.starts_with("code-review: not-enabled: code review is not enabled"),
    "{stderr}"
  );
  assert!(stderr.contains("project toolu.config.json"), "{stderr}");
}

#[test]
fn an_opted_in_run_still_fails_closed_and_uses_the_configured_effort() {
  let on = project(r#"{"codeReview":{"enabled":true,"effort":"small"}}"#).unwrap();
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
  assert!(message.contains("not built yet (#668, #669)"), "{message}");
}
