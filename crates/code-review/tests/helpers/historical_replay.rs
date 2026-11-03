//! Offline compatibility with host recordings keyed by their original prompt.

use toolu_code_review::eval::{
  ArmChoice, Fixtures, LiveOrReplayed, Mode, Plan, Setting, Store, run_case,
};
use toolu_code_review::review::Stage;

use super::{case, golden, host, replay};

fn historical_prompt(fixtures: &Fixtures) -> Result<String, String> {
  let bin = tempfile::tempdir().map_err(|err| err.to_string())?;
  host::host(bin.path(), "claude", &replay::answer()).map_err(|err| err.to_string())?;
  let base = replay::keyless(bin.path())?;
  let path = format!(
    "{}:{}",
    bin.path().display(),
    base.get("PATH").unwrap_or("")
  );
  let env = base
    .with("PATH", &path)
    .with("TOOLU_HOST_OVERRIDE", "claude")
    .with("TYPESAFE_API_KEY", "test-key");
  let mode = Mode::Live(None);
  let plan = Plan {
    env: &env,
    arms: ArmChoice::Baseline,
    settings: &[Setting::Small],
    mode: &mode,
    openrouter: toolu_openrouter::Config::default(),
    jev: None,
  };
  let result = run_case(fixtures, case(fixtures, "slice-underflow")?, &plan)?;
  if !result.unverified.is_empty() {
    return Err(format!("{:?}", result.unverified));
  }
  std::fs::read_to_string(bin.path().join("prompt.txt")).map_err(|err| err.to_string())
}

fn historical_run(
  fixtures: &Fixtures,
  prompt: &str,
  response: &str,
) -> Result<toolu_code_review::eval::CaseReport, String> {
  let dir = tempfile::tempdir().map_err(|err| err.to_string())?;
  let store = Store::new(dir.path());
  store.write(Stage::Review, prompt, response)?;
  let bin = tempfile::tempdir().map_err(|err| err.to_string())?;
  host::host(bin.path(), "claude", &replay::answer()).map_err(|err| err.to_string())?;
  let base = replay::keyless(bin.path())?;
  let path = format!(
    "{}:{}",
    bin.path().display(),
    base.get("PATH").unwrap_or("")
  );
  let env = base
    .with("PATH", &path)
    .with("TOOLU_HOST_OVERRIDE", "claude");
  let mode = Mode::Replay(store);
  let plan = Plan {
    env: &env,
    arms: ArmChoice::Baseline,
    settings: &[Setting::Small],
    mode: &mode,
    openrouter: toolu_openrouter::Config::default(),
    jev: None,
  };
  let result = run_case(fixtures, case(fixtures, "slice-underflow")?, &plan)?;
  if bin.path().join("prompt.txt").exists() {
    return Err("replay spawned a host".into());
  }
  Ok(result)
}

#[test]
fn historical_claude_and_codex_outputs_replay_from_prompt_keys() {
  let fixtures = golden().unwrap();
  let prompt = historical_prompt(&fixtures).unwrap();
  let claude = replay::answer();
  let envelope: serde_json::Value = serde_json::from_str(&claude).unwrap();
  let codex = envelope["result"].as_str().unwrap();
  for response in [claude.as_str(), codex] {
    let result = historical_run(&fixtures, &prompt, response).unwrap();
    assert!(result.unverified.is_empty(), "{:?}", result.unverified);
    assert_eq!(result.report.arms.len(), 1);
    let arm = &result.report.arms[0];
    assert_eq!(arm.tally.caught, 1);
    assert!(
      arm
        .calls
        .iter()
        .all(|call| call.live_or_replayed == LiveOrReplayed::Replayed)
    );
    assert_eq!(arm.usage.cost_usd, 0.0);
  }
}

#[test]
fn historical_malformed_or_error_outputs_fail_closed_after_lookup() {
  let fixtures = golden().unwrap();
  let prompt = historical_prompt(&fixtures).unwrap();
  for response in [
    "not json",
    r#"{"type":"result","is_error":true,"result":"{\"findings\":[]}"}"#,
    r#"{"type":"error","content":"{\"findings\":[]}","finish_reason":"stop","model":"forged","usage":{"prompt_tokens":1,"completion_tokens":1}}"#,
  ] {
    let result = historical_run(&fixtures, &prompt, response).unwrap();
    assert_eq!(result.report.arms.len(), 0);
    assert_ne!(result.unverified.len(), 0);
    assert!(
      result
        .unverified
        .iter()
        .all(|why| !why.contains("replay miss")),
      "{:?}",
      result.unverified
    );
  }
}
