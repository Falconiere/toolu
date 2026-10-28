use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::{Value, json};
use toolu_protocol::opencode::Hook;
use toolu_runtime::env::Env;

use super::{Bridge, matches, plan};

fn repo_plugin(name: &str) -> PathBuf {
  Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("../../plugins")
    .join(name)
}

fn hooks_json(name: &str) -> Value {
  let text = std::fs::read_to_string(repo_plugin(name).join("hooks/hooks.json")).unwrap();
  serde_json::from_str(&text).unwrap()
}

fn names(hook: Hook, subject: Option<&str>) -> Vec<String> {
  plan(hook, &hooks_json("toolu"), subject, Path::new("/p"))
    .unwrap()
    .into_iter()
    .map(|one| one.args.join(" "))
    .collect()
}

#[test]
fn a_tool_runs_only_the_entries_its_matcher_selects() {
  assert_eq!(
    names(Hook::ToolBefore, Some("Edit")),
    ["hook pre-tools --event PreToolUse --plugin-root /p"]
  );
  assert_eq!(
    names(Hook::ToolBefore, Some("Agent")),
    ["hook agent-tier --event PreToolUse --plugin-root /p"]
  );
  assert_eq!(
    names(Hook::ToolBefore, Some("mcp__context7__query")),
    ["hook mcp-tools --event PreToolUse --plugin-root /p"]
  );
  assert_eq!(names(Hook::ToolBefore, Some("Read")), Vec::<String>::new());
  assert_eq!(
    names(Hook::ToolAfter, Some("apply_patch")),
    ["hook post-tools --event PostToolUse --plugin-root /p"]
  );
}

#[test]
fn session_entries_follow_the_source_and_carry_their_label_and_timeout() {
  let planned = plan(
    Hook::SessionStart,
    &hooks_json("jev"),
    Some("startup"),
    Path::new("/p"),
  )
  .unwrap();
  let first = &planned[0];
  assert_eq!(
    first.args,
    [
      "jev",
      "hook",
      "session-start",
      "--event",
      "SessionStart",
      "--plugin-root",
      "/p"
    ]
  );
  assert_eq!(
    first.label,
    "jev plugin: toolu jev hook session-start on SessionStart"
  );
  assert!(first.timeout >= Duration::from_secs(1));
  assert_eq!(
    names(Hook::Prompt, None),
    ["hook user-prompt-submit --event UserPromptSubmit --plugin-root /p"]
  );
}

#[test]
fn a_foreign_command_or_missing_event_is_handled() {
  let foreign =
    json!({"hooks": {"PreToolUse": [{"hooks": [{"command": "echo hi", "timeout": 5}]}]}});
  let err = plan(Hook::ToolBefore, &foreign, Some("Bash"), Path::new("/p")).unwrap_err();
  assert!(err.contains("not a native toolu launcher"), "{err}");
  assert_eq!(
    plan(Hook::ToolAfter, &foreign, Some("Bash"), Path::new("/p")).unwrap(),
    []
  );
  assert_eq!(
    plan(Hook::ToolBefore, &Value::Null, None, Path::new("/p")).unwrap(),
    []
  );
}

#[test]
fn a_malformed_hooks_document_is_an_error_not_an_empty_plan() {
  let root = Path::new("/p");
  for doc in [
    json!([]),
    json!({"hooks": []}),
    json!({"hooks": {"PreToolUse": {}}}),
    json!({"hooks": {"PreToolUse": ["not a group"]}}),
    json!({"hooks": {"PreToolUse": [{"matcher": 1, "hooks": []}]}}),
    json!({"hooks": {"PreToolUse": [{"hooks": {}}]}}),
  ] {
    assert!(
      plan(Hook::ToolBefore, &doc, Some("Bash"), root).is_err(),
      "{doc}"
    );
  }
  for empty in [
    json!({}),
    json!({"hooks": {}}),
    json!({"hooks": {"PreToolUse": [{}]}}),
  ] {
    assert_eq!(
      plan(Hook::ToolBefore, &empty, Some("Bash"), root).unwrap(),
      [],
      "{empty}"
    );
  }
}

#[test]
fn matchers_search_like_claude_code() {
  assert!(matches("", Some("Bash")));
  assert!(matches("*", Some("Bash")));
  assert!(matches("Edit|Write", Some("Write")));
  assert!(matches("mcp__", Some("mcp__server__tool")));
  assert!(!matches("Edit|Write", Some("Read")));
  assert!(matches("Edit", None));
  assert!(matches("(", Some("(")));
  assert!(!matches("(", Some("x")));
}

fn bridge<'a>(hook: Hook, root: &'a Path, exe: &str) -> Bridge<'a> {
  Bridge {
    hook,
    root,
    exe: PathBuf::from(exe),
    env: Env::from_pairs([("PATH", "/usr/bin:/bin")]),
  }
}

#[test]
fn each_entry_runs_as_the_binary_with_its_argv_in_the_call_directory() {
  let root = repo_plugin("jev");
  let call = json!({"directory": "/", "event": {"sessionID": "s"}}).to_string();
  let reply: Value =
    serde_json::from_str(&bridge(Hook::SessionStart, &root, "/bin/echo").run(Some(&call))).unwrap();
  assert_eq!(reply["action"], "continue");
  let context = reply["context"].as_str().unwrap();
  assert!(
    context.contains("jev hook session-start --event SessionStart"),
    "{context}"
  );
  assert!(context.contains("jev hook check-binary"), "{context}");
}

#[test]
fn a_failure_before_a_tool_denies_and_elsewhere_is_a_message() {
  let root = repo_plugin("toolu");
  let call =
    json!({"directory": "/", "event": {"tool": "edit", "input": {"path": "a"}}}).to_string();
  let denied: Value =
    serde_json::from_str(&bridge(Hook::ToolBefore, &root, "/usr/bin/false").run(Some(&call)))
      .unwrap();
  assert_eq!(denied["action"], "deny");
  assert!(
    denied["reason"]
      .as_str()
      .unwrap()
      .contains("ended with status 1")
  );
  let unread: Value =
    serde_json::from_str(&bridge(Hook::Prompt, &root, "/usr/bin/false").run(None)).unwrap();
  assert_eq!(unread["action"], "continue");
  assert!(
    unread["message"]
      .as_str()
      .unwrap()
      .contains("could not be read")
  );
  let missing: Value =
    serde_json::from_str(&bridge(Hook::ToolBefore, &root, "/nonexistent/toolu").run(Some(&call)))
      .unwrap();
  assert_eq!(missing["action"], "deny");
  assert!(
    missing["reason"]
      .as_str()
      .unwrap()
      .contains("could not run")
  );
}

#[test]
fn a_plugin_without_hooks_continues_and_a_broken_hooks_file_fails() {
  let dir = tempfile::tempdir().unwrap();
  let call = json!({"event": {"tool": "shell"}}).to_string();
  let none: Value =
    serde_json::from_str(&bridge(Hook::ToolBefore, dir.path(), "/usr/bin/false").run(Some(&call)))
      .unwrap();
  assert_eq!(none, json!({"action": "continue"}));
  std::fs::create_dir_all(dir.path().join("hooks")).unwrap();
  std::fs::write(dir.path().join("hooks/hooks.json"), "{").unwrap();
  let broken: Value =
    serde_json::from_str(&bridge(Hook::ToolBefore, dir.path(), "/usr/bin/false").run(Some(&call)))
      .unwrap();
  assert_eq!(broken["action"], "deny");
  assert!(broken["reason"].as_str().unwrap().contains("is not JSON"));
}

#[test]
fn a_call_without_an_existing_directory_is_refused_before_a_tool() {
  let root = repo_plugin("toolu");
  for directory in [json!(null), json!("/nonexistent-toolu-dir")] {
    let call = json!({"directory": directory, "event": {"tool": "edit", "input": {"path": "a"}}});
    let reply: Value = serde_json::from_str(
      &bridge(Hook::ToolBefore, &root, "/bin/echo").run(Some(&call.to_string())),
    )
    .unwrap();
    assert_eq!(reply["action"], "deny", "{reply}");
    assert!(
      reply["reason"]
        .as_str()
        .unwrap()
        .contains("names no existing directory"),
      "{reply}"
    );
  }
  let missing_workdir = json!({"directory": "/", "event": {"tool": "shell",
    "input": {"command": "ls", "workdir": "/nonexistent-toolu-dir"}}});
  let reply: Value = serde_json::from_str(
    &bridge(Hook::ToolBefore, &root, "/usr/bin/true").run(Some(&missing_workdir.to_string())),
  )
  .unwrap();
  assert_eq!(reply["action"], "continue", "{reply}");
}
