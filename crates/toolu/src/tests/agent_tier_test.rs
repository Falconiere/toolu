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

fn observed_muse_payload(tool: &str) -> serde_json::Value {
  let path = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../docs/toolu/evidence/2026-10-19-muse-tool-inventory.json"
  );
  let inventory: serde_json::Value =
    serde_json::from_str(&std::fs::read_to_string(path).expect("inventory")).expect("JSON");
  inventory["tools"][tool]["pre_example"].clone()
}

#[test]
fn observed_muse_spawn_records_role_without_inventing_child_model() {
  let (dir, env) = fixture();
  let env = env.with("TOOLU_HOST_OVERRIDE", "muse");
  let payload = observed_muse_payload("subagent_spawn").to_string();
  assert_eq!(evaluate(&payload, env, dir.path()), silent());
  let telemetry = dir.path().join(".agents/tmp/telemetry/feat_x.jsonl");
  let line = std::fs::read_to_string(telemetry).expect("delegation telemetry");
  let event: serde_json::Value = serde_json::from_str(line.trim()).expect("event JSON");
  assert_eq!(event["event"], "delegation");
  assert_eq!(event["subagent_type"], "responder");
  assert!(event["model"].is_null());
  assert!(event["reasoning_effort"].is_null());
}

#[test]
fn muse_spawn_with_structured_model_and_effort_emits_muse_advice() {
  let (dir, env) = fixture();
  let env = env.with("TOOLU_HOST_OVERRIDE", "muse");
  let ledger = dir.path().join(".agents/tmp/plan-ledger/feat_x.json");
  std::fs::create_dir_all(ledger.parent().expect("parent")).expect("ledger dir");
  std::fs::write(
    &ledger,
    r#"{"steps":[{"id":"s1","status":"running","model":"opus"}],"next":"s1"}"#,
  )
  .expect("ledger");
  let mut payload = observed_muse_payload("subagent_spawn");
  payload["tool_input"]["model"] = serde_json::json!("sonnet");
  payload["tool_input"]["effort"] = serde_json::json!("high");
  let out = evaluate(&payload.to_string(), env.clone(), dir.path());
  assert_eq!(out.exit, Exit::Success);
  let advice: serde_json::Value =
    serde_json::from_str(&out.stdout.expect("advice")).expect("Muse JSON");
  assert!(
    advice["systemMessage"]
      .as_str()
      .expect("message")
      .contains("opus")
  );
  assert!(advice.get("hookSpecificOutput").is_none());
  let telemetry = dir.path().join(".agents/tmp/telemetry/feat_x.jsonl");
  let line = std::fs::read_to_string(telemetry).expect("delegation telemetry");
  let event: serde_json::Value = serde_json::from_str(line.trim()).expect("event JSON");
  assert_eq!(event["model"], "sonnet");
  assert_eq!(event["subagent_type"], "responder");
  assert_eq!(event["reasoning_effort"], "high");
  payload["tool_input"]["model"] = serde_json::json!("opus");
  let matching = evaluate(&payload.to_string(), env, dir.path());
  assert_eq!(matching.stdout, None);
}

#[test]
fn muse_internal_tool_does_not_write_delegation_telemetry() {
  let (dir, env) = fixture();
  let env = env.with("TOOLU_HOST_OVERRIDE", "muse");
  let payload = observed_muse_payload("submit_reminder_decision").to_string();
  assert_eq!(evaluate(&payload, env, dir.path()), silent());
  assert!(!dir.path().join(".agents/tmp/telemetry").exists());
}
