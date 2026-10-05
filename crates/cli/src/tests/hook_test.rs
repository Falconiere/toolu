use std::path::PathBuf;

use super::run;
use crate::args::HookRequest;
use crate::{Context, VERSION};

const STARTUP: &str = r#"{"hook_event_name":"SessionStart","source":"startup"}"#;

fn startup() -> String {
  STARTUP.to_owned()
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

fn context() -> Context<'static> {
  Context {
    exe: Some(PathBuf::from("/usr/local/bin/toolu")),
    stdin: &startup,
  }
}

fn message(stdout: Option<String>) -> String {
  let json: serde_json::Value = serde_json::from_str(&stdout.unwrap()).unwrap();
  json["systemMessage"].as_str().unwrap().to_owned()
}

#[test]
fn the_same_version_prints_only_the_diagnostic() {
  let dir = plugin(VERSION, "1");
  let root = dir.path().to_str().unwrap();
  let outcome = run(
    &request("session-start", Some("SessionStart"), Some(root)),
    &context(),
  );
  assert_eq!(outcome.code, 0);
  assert_eq!(
    message(outcome.stdout),
    format!("toolu runtime: native {VERSION} at /usr/local/bin/toolu")
  );
}

#[test]
fn semver_skew_advises_at_session_start_only() {
  let dir = plugin("999.0.0", "1");
  let root = dir.path().to_str().unwrap();
  let start = run(
    &request("session-start", Some("SessionStart"), Some(root)),
    &context(),
  );
  let text = message(start.stdout);
  assert!(text.starts_with(&format!(
    "toolu {VERSION} is older than the toolu plugin 999.0.0"
  )));
  assert!(text.contains("upgrade it: curl -fsSL"));
  assert!(text.ends_with(&format!(
    "\ntoolu runtime: native {VERSION} at /usr/local/bin/toolu"
  )));
  let pre = run(
    &request("session-start", Some("PreToolUse"), Some(root)),
    &context(),
  );
  assert_eq!(pre.code, 0);
  assert!(!message(pre.stdout).contains("older than"));
}

#[test]
fn a_protocol_mismatch_blocks_enforcing_and_reports_context_events() {
  let dir = plugin(VERSION, "2");
  let root = dir.path().to_str().unwrap();
  let pre = run(
    &request("session-start", Some("PreToolUse"), Some(root)),
    &context(),
  );
  assert_eq!(pre.code, 2);
  assert_eq!(pre.stdout, None);
  assert!(
    pre
      .stderr
      .unwrap()
      .starts_with("blocked: toolu plugin: hook protocol 2 needs a newer")
  );
  let start = run(
    &request("session-start", Some("SessionStart"), Some(root)),
    &context(),
  );
  assert_eq!(start.code, 0);
  assert!(message(start.stdout).starts_with("toolu plugin: hook protocol 2 needs a newer"));
}

#[test]
fn an_empty_plugin_root_is_a_mismatch_not_a_bypass() {
  let outcome = run(
    &request("session-start", Some("PreToolUse"), Some("")),
    &context(),
  );
  assert_eq!(outcome.code, 2);
  assert!(outcome.stderr.unwrap().contains("the plugin root is empty"));
}

#[test]
fn without_a_plugin_root_the_prelude_is_skipped() {
  let outcome = run(
    &request("session-start", Some("SessionStart"), None),
    &context(),
  );
  assert_eq!(outcome.code, 0);
  assert!(message(outcome.stdout).starts_with("toolu runtime: native"));
}

#[test]
fn an_unknown_hook_blocks_enforcing_and_missing_events_and_reports_context_ones() {
  for event in [Some("PreToolUse"), None] {
    let outcome = run(&request("pre-tools", event, None), &context());
    assert_eq!(outcome.code, 2, "{event:?}");
    assert!(
      outcome
        .stderr
        .unwrap()
        .contains("has no hook pre-tools; upgrade it: curl")
    );
  }
  let context_event = run(
    &request("pre-compact", Some("PreCompact"), None),
    &context(),
  );
  assert_eq!(context_event.code, 0);
  assert!(message(context_event.stdout).contains("has no hook pre-compact"));
}

#[test]
fn a_session_start_with_nothing_to_say_prints_nothing() {
  let quiet = || "{}".to_owned();
  let context = Context {
    exe: None,
    stdin: &quiet,
  };
  let outcome = run(
    &request("session-start", Some("SessionStart"), None),
    &context,
  );
  assert_eq!(outcome.stdout, None);
  assert_eq!(outcome.code, 0);
}
