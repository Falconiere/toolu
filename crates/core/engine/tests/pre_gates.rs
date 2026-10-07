//! Real pre-tool dispatch cases for the command-driven gates of #420.

#[path = "helpers/hook.rs"]
mod hook;
#[path = "helpers/sandbox.rs"]
mod sandbox;

use hook::Hook;
use sandbox::{Res, write};
use serde_json::json;
use toolu_engine::Phase;
use toolu_engine::builtins::PRE_TOOL;
use toolu_protocol::host::Host;

fn bash(command: &str) -> String {
  json!({
    "session_id": "s1", "hook_event_name": "PreToolUse",
    "tool_name": "Bash", "tool_input": {"command": command},
  })
  .to_string()
}

fn with_settings() -> Res<Hook> {
  let mut hook = Hook::new(Host::Claude)?;
  let settings = hook.sb.path("settings");
  std::fs::create_dir_all(&settings).map_err(|err| err.to_string())?;
  hook.extra.push((
    "TOOLU_SETTINGS_DIR".to_owned(),
    settings.to_string_lossy().into_owned(),
  ));
  Ok(hook)
}

fn block_modes(hook: &Hook) -> Res<()> {
  write(
    &hook.sb.path("project/.claude/toolu.config.json"),
    &json!({"version": 1, "gates": {
      "bashCommands": {"mode": "block"},
      "commitGate": {"mode": "block"},
      "qualityGate": {"mode": "block"},
    }})
    .to_string(),
  )
}

#[test]
fn a_denied_simple_command_after_cd_is_blocked() {
  let hook = with_settings().expect("sandbox");
  block_modes(&hook).expect("config");
  write(&hook.sb.path("settings/bash-denylist.txt"), "node -e\n").expect("deny list");
  assert!(hook.dir(Phase::Pre).starts_with(hook.config_root()));
  hook
    .sh(Phase::Pre, "x@t__later.sh", "exit 2")
    .expect("registry module");
  let got = hook.run(Phase::Pre, &bash("cd /tmp && node -e 1"), PRE_TOOL, &[]);
  assert!(
    got
      .result
      .stdout
      .contains("Command blocked by deny rule: node -e")
  );
}

#[test]
fn an_unknown_static_commit_prefix_is_blocked() {
  let hook = with_settings().expect("sandbox");
  block_modes(&hook).expect("config");
  write(&hook.sb.path("settings/commit-prefixes.txt"), "feat\nfix\n").expect("prefixes");
  let got = hook.run(
    Phase::Pre,
    &bash("git commit -m 'wibble: x'"),
    PRE_TOOL,
    &[],
  );
  assert!(
    got
      .result
      .stdout
      .contains("Unknown Conventional Commits prefix: \\\"wibble\\\"")
  );
}

#[test]
fn failing_legacy_quality_state_blocks_a_commit() {
  let hook = with_settings().expect("sandbox");
  block_modes(&hook).expect("config");
  write(
    &hook.sb.path("project/.claude/tmp/quality-gate-status.json"),
    "{\"status\":\"failing\",\"reason\":\"lint failed\",\"violations\":\"a.ts:1\"}\n",
  )
  .expect("gate file");
  let got = hook.run(Phase::Pre, &bash("git commit -m 'feat: x'"), PRE_TOOL, &[]);
  assert!(got.result.stdout.contains("BLOCKED: quality gate failing"));
  assert!(got.result.stdout.contains("a.ts:1"));
}
