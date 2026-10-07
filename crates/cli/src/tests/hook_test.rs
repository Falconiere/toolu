use std::path::{Path, PathBuf};

use toolu_runtime::cli::Outcome;
use toolu_runtime::env::Env;

use super::run;
use crate::fast::HookRequest;
use crate::{Context, VERSION};

const STARTUP: &str = r#"{"hook_event_name":"SessionStart","source":"startup"}"#;

fn startup() -> std::io::Result<String> {
  toolu_protocol::stdin::read_all(STARTUP.as_bytes())
}

fn plugin(version: &str, protocol: &str) -> tempfile::TempDir {
  let dir = tempfile::tempdir().unwrap();
  std::fs::create_dir_all(dir.path().join(".claude-plugin")).unwrap();
  std::fs::write(
    dir.path().join(".claude-plugin/plugin.json"),
    format!(r#"{{"name":"toolu","version":"{version}","hookProtocol":{protocol}}}"#),
  )
  .unwrap();
  dir
}

fn request(name: &str, event: Option<&str>, root: Option<&str>) -> HookRequest {
  HookRequest {
    plugin: "toolu".to_owned(),
    name: name.to_owned(),
    event: event.map(str::to_owned),
    plugin_root: root.map(str::to_owned),
  }
}

/// Run `request` as the binary installed at `/usr/local/bin/toolu`, on a startup payload.
fn run_hook(request: &HookRequest) -> Outcome {
  execute(request, startup, || {
    Some(PathBuf::from("/usr/local/bin/toolu"))
  })
}

fn execute(
  request: &HookRequest,
  stdin: impl Fn() -> std::io::Result<String>,
  exe: impl Fn() -> Option<PathBuf>,
) -> Outcome {
  let dir = isolated_dir();
  let env = isolated_env(dir.path());
  let cwd = dir.path().to_path_buf();
  let env_fn = || env.clone();
  let cwd_fn = || cwd.clone();
  run(
    request,
    &Context {
      exe: &exe,
      stdin: &stdin,
      env: &env_fn,
      cwd: &cwd_fn,
    },
  )
}

fn isolated_dir() -> tempfile::TempDir {
  let dir = tempfile::tempdir().unwrap();
  std::fs::create_dir_all(dir.path().join("home")).unwrap();
  std::fs::create_dir_all(dir.path().join("cfg")).unwrap();
  std::fs::create_dir_all(dir.path().join("bin")).unwrap();
  dir
}

fn isolated_env(root: &Path) -> Env {
  Env::from_pairs([
    ("HOME", root.join("home").to_str().unwrap()),
    ("PATH", root.join("bin").to_str().unwrap()),
    ("TOOLU_CONFIG_DIR", root.join("cfg").to_str().unwrap()),
    ("CLAUDE_PROJECT_DIR", root.to_str().unwrap()),
  ])
}

fn message(stdout: Option<String>) -> String {
  let json: serde_json::Value = serde_json::from_str(&stdout.unwrap()).unwrap();
  json["systemMessage"].as_str().unwrap().to_owned()
}

#[test]
fn the_same_version_prints_the_startup_line() {
  let dir = plugin(VERSION, "1");
  let root = dir.path().to_str().unwrap();
  let outcome = run_hook(&request("session-start", Some("SessionStart"), Some(root)));
  assert_eq!(outcome.exit.code(), 0);
  assert_eq!(
    message(outcome.stdout),
    format!("Toolu is on!\ntoolu runtime: native {VERSION} at /usr/local/bin/toolu")
  );
}

#[test]
fn semver_skew_advises_at_session_start_only() {
  let dir = plugin("999.0.0", "1");
  let root = dir.path().to_str().unwrap();
  let start = run_hook(&request("session-start", Some("SessionStart"), Some(root)));
  let text = message(start.stdout);
  assert!(text.starts_with(&format!(
    "toolu {VERSION} is older than the toolu plugin 999.0.0"
  )));
  assert!(text.contains("upgrade it: curl -fsSL"));
  assert!(text.ends_with(&format!(
    "\ntoolu runtime: native {VERSION} at /usr/local/bin/toolu"
  )));
  let pre = run_hook(&request("session-start", Some("PreToolUse"), Some(root)));
  assert_eq!(pre.exit.code(), 0);
  assert!(!message(pre.stdout).contains("older than"));
}

#[test]
fn a_protocol_mismatch_blocks_enforcing_and_reports_context_events() {
  let dir = plugin(VERSION, "2");
  let root = dir.path().to_str().unwrap();
  let pre = run_hook(&request("session-start", Some("PreToolUse"), Some(root)));
  assert_eq!(pre.exit.code(), 2);
  assert_eq!(pre.stdout, None);
  assert!(
    pre
      .stderr
      .unwrap()
      .starts_with("blocked: toolu plugin: hook protocol 2 needs a newer")
  );
  let start = run_hook(&request("session-start", Some("SessionStart"), Some(root)));
  assert_eq!(start.exit.code(), 0);
  assert!(message(start.stdout).starts_with("toolu plugin: hook protocol 2 needs a newer"));
}

#[test]
fn an_empty_plugin_root_is_a_mismatch_not_a_bypass() {
  let outcome = run_hook(&request("session-start", Some("PreToolUse"), Some("")));
  assert_eq!(outcome.exit.code(), 2);
  assert!(outcome.stderr.unwrap().contains("the plugin root is empty"));
}

#[test]
fn without_a_plugin_root_the_prelude_is_skipped() {
  let outcome = run_hook(&request("session-start", Some("SessionStart"), None));
  assert_eq!(outcome.exit.code(), 0);
  assert!(message(outcome.stdout).starts_with("Toolu is on!"));
}

#[test]
fn an_unknown_hook_blocks_enforcing_and_missing_events_and_reports_context_ones() {
  for event in [Some("PreToolUse"), None] {
    let outcome = run_hook(&request("no-such-hook", event, None));
    assert_eq!(outcome.exit.code(), 2, "{event:?}");
    assert!(
      outcome
        .stderr
        .unwrap()
        .contains("has no hook no-such-hook; upgrade it: curl")
    );
  }
  let context_event = run_hook(&request("session-end", Some("SessionEnd"), None));
  assert_eq!(context_event.exit.code(), 0);
  assert!(message(context_event.stdout).contains("has no hook session-end"));
}

#[test]
fn pre_compact_prints_nothing() {
  let outcome = run_hook(&request("pre-compact", Some("PreCompact"), None));
  assert_eq!(outcome.exit.code(), 0);
  assert_eq!(outcome.stdout, None);
}

#[test]
fn an_empty_payload_is_startup() {
  let outcome = execute(
    &request("session-start", Some("SessionStart"), None),
    || Ok("{}".to_owned()),
    || None,
  );
  assert_eq!(outcome.exit.code(), 0);
  assert_eq!(
    message(outcome.stdout),
    format!("Toolu is on!\ntoolu runtime: native {VERSION} at an unknown path")
  );
}

#[test]
fn an_unreadable_payload_is_reported_not_swallowed() {
  let outcome = execute(
    &request("session-start", Some("SessionStart"), None),
    || Err(std::io::Error::other("stdin is closed")),
    || None,
  );
  assert_eq!(outcome.exit.code(), 0);
  assert_eq!(
    message(outcome.stdout),
    format!(
      "toolu runtime: native {VERSION}, but the hook payload could not be read: stdin is closed"
    )
  );
}

#[test]
fn agent_tier_fails_open_when_payload_is_unreadable() {
  let outcome = execute(
    &request("agent-tier", Some("PreToolUse"), None),
    || Err(std::io::Error::other("stdin is closed")),
    || None,
  );
  assert_eq!(outcome.exit.code(), 0);
  assert_eq!(outcome.stdout, None);
  assert_eq!(outcome.stderr, None);
}

#[test]
fn mcp_tools_routes_a_non_mcp_payload_to_silent_success() {
  let outcome = execute(
    &request("mcp-tools", Some("PreToolUse"), None),
    || Ok(r#"{"tool_name":"Bash"}"#.to_owned()),
    || None,
  );
  assert_eq!(outcome.exit.code(), 0);
  assert_eq!(outcome.stdout, None);
  assert_eq!(outcome.stderr, None);
}

#[test]
fn another_plugins_session_start_advises_and_never_blocks() {
  let other = HookRequest {
    plugin: "statusline".to_owned(),
    name: "session-start".to_owned(),
    event: Some("SessionStart".to_owned()),
    plugin_root: None,
  };
  let outcome = run_hook(&other);
  assert_eq!(outcome.exit.code(), 0);
  assert_eq!(outcome.stderr, None);
  assert!(message(outcome.stdout).starts_with(&format!(
    "statusline plugin: toolu {VERSION} has no hook session-start"
  )));
}

#[test]
fn an_unknown_jev_hook_is_still_reported() {
  let mut jev = request("not-a-hook", Some("SessionStart"), None);
  jev.plugin = "jev".to_owned();
  let outcome = run_hook(&jev);
  assert_eq!((outcome.exit.code(), outcome.stderr.is_none()), (0, true));
  assert!(message(outcome.stdout).contains("has no hook not-a-hook"));
}

#[test]
fn toolus_pre_and_post_tools_are_the_engines_tool_hooks() {
  use toolu_hub::tool_hook::Phase;
  assert_eq!(
    super::tool_phase(&request("pre-tools", None, None)),
    Some(Phase::Pre)
  );
  assert_eq!(
    super::tool_phase(&request("post-tools", None, None)),
    Some(Phase::Post)
  );
  assert_eq!(
    super::tool_phase(&request("session-start", None, None)),
    None
  );
  let mut other = request("pre-tools", None, None);
  other.plugin = "jev".to_owned();
  assert_eq!(super::tool_phase(&other), None);
}

#[test]
fn a_protocol_mismatch_blocks_a_tool_hook_before_it_runs() {
  let root = plugin(VERSION, "999");
  let path = root.path().to_str().unwrap();
  let out = run_hook(&request("pre-tools", Some("PreToolUse"), Some(path)));
  assert_eq!(out.exit, toolu_protocol::exit::Exit::Blocked);
  assert!(
    out
      .stderr
      .unwrap()
      .starts_with("blocked: toolu plugin: hook protocol 999 needs a newer")
  );
}
