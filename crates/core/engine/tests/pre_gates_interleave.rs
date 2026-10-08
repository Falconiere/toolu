//! TypeScript and Rust hooks share the quality gate file through a real git lifecycle.

#[path = "helpers/hook.rs"]
mod hook;
#[path = "helpers/sandbox.rs"]
mod sandbox;

use std::io::Write as _;
use std::process::{Command, Stdio};

use hook::Hook;
use serde_json::{Value, json};
use toolu_engine::Phase;
use toolu_engine::builtins::{POST_TOOL, PRE_TOOL};
use toolu_protocol::host::Host;

const PRE_BUNDLE: &str = concat!(
  env!("CARGO_MANIFEST_DIR"),
  "/../../../plugins/toolu/hooks/dist/pre-tools.js"
);
const POST_BUNDLE: &str = concat!(
  env!("CARGO_MANIFEST_DIR"),
  "/../../../plugins/toolu/hooks/dist/post-tools.js"
);

fn pre_commit() -> String {
  json!({
    "session_id": "s1", "hook_event_name": "PreToolUse",
    "tool_name": "Bash", "tool_input": {"command": "git commit -m 'feat: x'"},
  })
  .to_string()
}

fn post_quality(code: i64) -> String {
  json!({
    "session_id": "s1", "hook_event_name": "PostToolUse",
    "tool_name": "Bash", "tool_input": {"command": "bun test"},
    "tool_response": {"metadata": {"exit_code": code}},
  })
  .to_string()
}

fn typescript(hook: &Hook, bundle: &str, stdin: &str) -> Result<String, String> {
  let path = std::env::var("PATH").unwrap_or_default();
  let mut child = Command::new("bun")
    .arg(bundle)
    .current_dir(hook.sb.path("project"))
    .env_clear()
    .env("PATH", path)
    .env("HOME", hook.sb.text("home"))
    .env("CLAUDE_PROJECT_DIR", hook.sb.text("project"))
    .env("TOOLU_CONFIG_DIR", hook.config_root())
    .env("TOOLU_SETTINGS_DIR", hook.sb.path("settings"))
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .map_err(|err| err.to_string())?;
  child
    .stdin
    .take()
    .ok_or("missing stdin".to_owned())?
    .write_all(stdin.as_bytes())
    .map_err(|err| err.to_string())?;
  let result = child.wait_with_output().map_err(|err| err.to_string())?;
  if !result.status.success() {
    return Err(String::from_utf8_lossy(&result.stderr).into_owned());
  }
  String::from_utf8(result.stdout).map_err(|err| err.to_string())
}

fn sandbox() -> Result<Hook, String> {
  let mut hook = Hook::new(Host::Claude)?;
  let settings = hook.sb.path("settings");
  std::fs::create_dir_all(&settings).map_err(|err| err.to_string())?;
  hook.extra.push((
    "TOOLU_SETTINGS_DIR".to_owned(),
    settings.display().to_string(),
  ));
  let config = hook.sb.path("project/.claude/toolu.config.json");
  sandbox::write(
    &config,
    "{\"version\":1,\"gates\":{\"qualityGate\":{\"mode\":\"block\"}}}",
  )?;
  if !hook.dir(Phase::Post).starts_with(hook.config_root()) {
    return Err("post registry directory is outside config root".to_owned());
  }
  hook.sh(Phase::Post, "x@t__noop.sh", "exit 0")?;
  Ok(hook)
}

fn gate_status(hook: &Hook) -> Option<String> {
  let file = hook.sb.path("project/.claude/tmp/quality-gate-status.json");
  let value: Value = serde_json::from_slice(&std::fs::read(file).ok()?).ok()?;
  value.get("status")?.as_str().map(str::to_owned)
}

#[test]
fn typescript_failure_and_clear_are_seen_by_rust_pre_tool() {
  let hook = sandbox().expect("sandbox");
  typescript(&hook, POST_BUNDLE, &post_quality(1)).expect("TS failure");
  assert_eq!(gate_status(&hook).as_deref(), Some("failing"));
  let blocked = hook.run(Phase::Pre, &pre_commit(), PRE_TOOL, &[]);
  assert!(
    blocked
      .result
      .stdout
      .contains("BLOCKED: quality gate failing")
  );
  typescript(&hook, POST_BUNDLE, &post_quality(0)).expect("TS clear");
  assert_eq!(gate_status(&hook).as_deref(), Some("passing"));
  let allowed = hook.run(Phase::Pre, &pre_commit(), PRE_TOOL, &[]);
  assert!(
    !allowed
      .result
      .stdout
      .contains("BLOCKED: quality gate failing")
  );
}

#[test]
fn rust_failure_and_clear_are_seen_by_typescript_pre_tool() {
  let hook = sandbox().expect("sandbox");
  let failed = hook.run(Phase::Post, &post_quality(1), POST_TOOL, &[]);
  assert_eq!(failed.result.exit_code, 0);
  assert_eq!(gate_status(&hook).as_deref(), Some("failing"));
  let blocked = typescript(&hook, PRE_BUNDLE, &pre_commit()).expect("TS pre");
  assert!(blocked.contains("BLOCKED: quality gate failing"));
  let cleared = hook.run(Phase::Post, &post_quality(0), POST_TOOL, &[]);
  assert_eq!(cleared.result.exit_code, 0);
  assert_eq!(gate_status(&hook).as_deref(), Some("passing"));
  let allowed = typescript(&hook, PRE_BUNDLE, &pre_commit()).expect("TS pre after clear");
  assert!(!allowed.contains("BLOCKED: quality gate failing"));
}
