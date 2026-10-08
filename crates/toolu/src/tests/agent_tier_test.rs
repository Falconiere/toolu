use super::*;

#[test]
fn malformed_payload_fails_open() {
  let out = evaluate("{", Env::process(), Path::new("."));
  assert_eq!(out, silent());
}

fn fixture() -> (tempfile::TempDir, Env) {
  use toolu_runtime::process::{Spec, run};
  let dir = tempfile::tempdir().expect("tempdir");
  let mut git = Spec::new(["git", "init", "-q", "-b", "feat/x"]);
  git.cwd = Some(dir.path().to_path_buf());
  git.env = Some(Env::process());
  assert_eq!(run(&git).expect("git init").exit_code, 0);
  let mut commit = Spec::new([
    "git",
    "-c",
    "user.name=Test",
    "-c",
    "user.email=t@example.com",
    "commit",
    "--allow-empty",
    "-q",
    "-m",
    "base",
  ]);
  commit.cwd = Some(dir.path().to_path_buf());
  commit.env = Some(Env::process());
  assert_eq!(run(&commit).expect("git commit").exit_code, 0);
  let env = Env::process()
    .with("TOOLU_HOST_OVERRIDE", "claude")
    .with("TOOLU_PROJECT_DIR", &dir.path().display().to_string())
    .with(
      "TOOLU_CONFIG_DIR",
      &dir.path().join("user").display().to_string(),
    );
  (dir, env)
}

#[test]
fn real_ledger_step_controls_delegation_advice_and_telemetry() {
  let (dir, env) = fixture();
  let ledger = dir.path().join(".claude/tmp/plan-ledger/feat_x.json");
  std::fs::create_dir_all(ledger.parent().expect("parent")).expect("ledger dir");
  std::fs::write(
    &ledger,
    r#"{"steps":[{"id":"s1","status":"running","model":"opus"}],"next":"s1"}"#,
  )
  .expect("ledger");
  let payload =
    r#"{"tool_name":"Agent","tool_input":{"model":"sonnet","subagent_type":"reviewer"}}"#;
  let out = evaluate(payload, env, dir.path());
  assert_eq!(out.exit, Exit::Success);
  let advice = out.stdout.expect("advice");
  assert!(advice.contains("opus") && advice.contains("sonnet"));
  let telemetry = dir.path().join(".claude/tmp/telemetry/feat_x.jsonl");
  let line = std::fs::read_to_string(telemetry).expect("telemetry");
  assert!(line.contains("\"step_id\":\"s1\""));
  assert!(line.contains("\"subagent_type\":\"reviewer\""));
}

#[test]
fn explicit_block_mode_denies_but_matching_model_is_silent() {
  let (dir, env) = fixture();
  let ledger = dir.path().join(".claude/tmp/plan-ledger/feat_x.json");
  std::fs::create_dir_all(ledger.parent().expect("parent")).expect("ledger dir");
  std::fs::write(
    &ledger,
    r#"{"steps":[{"id":"s1","model":"opus"}],"next":"s1"}"#,
  )
  .expect("ledger");
  let config = dir.path().join(".claude/toolu.config.json");
  std::fs::write(config, r#"{"version":1,"agentTier":{"mode":"block"}}"#).expect("config");
  let mismatch = r#"{"tool_name":"Agent","tool_input":{"model":"sonnet"}}"#;
  let denied = evaluate(mismatch, env.clone(), dir.path());
  assert!(denied.stdout.expect("deny").contains("\"deny\""));
  let matching = r#"{"tool_name":"Agent","tool_input":{"model":"opus"}}"#;
  let allowed = evaluate(matching, env, dir.path());
  assert_eq!(allowed.stdout, None);
}
