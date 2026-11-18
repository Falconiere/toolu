//! Real git pre-tool checks for the C workflow gates.

#[path = "helpers/hook.rs"]
mod hook;
#[path = "helpers/sandbox.rs"]
mod sandbox;

use std::process::Command;

use hook::Hook;
use sandbox::{Res, write};
use serde_json::json;
use toolu_engine::Phase;
use toolu_engine::builtins::PRE_TOOL;
use toolu_protocol::host::Host;

fn git(hook: &Hook, args: &[&str]) -> Res<()> {
  let status = Command::new("git")
    .args(args)
    .current_dir(hook.sb.path("project"))
    .status()
    .map_err(|err| err.to_string())?;
  status
    .success()
    .then_some(())
    .ok_or_else(|| format!("git {args:?} failed"))
}

fn branch() -> Res<Hook> {
  let hook = Hook::new(Host::Claude)?;
  git(&hook, &["config", "user.email", "test@example.com"])?;
  git(&hook, &["config", "user.name", "Test"])?;
  write(&hook.sb.path("project/base.txt"), "base\n")?;
  git(&hook, &["add", "base.txt"])?;
  git(&hook, &["commit", "-qm", "base"])?;
  git(&hook, &["branch", "-M", "main"])?;
  git(&hook, &["checkout", "-qb", "feat/example"])?;
  write(&hook.sb.path("project/feature.txt"), "feature\n")?;
  git(&hook, &["add", "feature.txt"])?;
  git(&hook, &["commit", "-qm", "feature"])?;
  write(
    &hook.sb.path("project/.claude/toolu.config.json"),
    &json!({"version": 1, "gates": {"preset": "strict"}}).to_string(),
  )?;
  Ok(hook)
}

#[test]
fn push_without_review_is_denied_on_the_target_branch() {
  let hook = branch().expect("git branch");
  assert!(hook.dir(Phase::Pre).starts_with(hook.config_root()));
  hook
    .sh(Phase::Pre, "x@t__later.sh", "exit 0")
    .expect("registry module");
  let payload = json!({
    "session_id": "s1", "hook_event_name": "PreToolUse",
    "tool_name": "Bash", "tool_input": {"command": "git push"},
  });
  let got = hook.run(Phase::Pre, &payload.to_string(), PRE_TOOL, &[]);
  assert!(
    got
      .result
      .stdout
      .contains("Code review required before push")
  );
}

#[test]
fn qwen_push_review_targets_the_shell_directory() {
  let target = branch().expect("target branch");
  let hook = Hook::new(Host::Qwen).expect("Qwen workspace");
  write(
    &hook.sb.path("project/.qwen/toolu.config.json"),
    &json!({"version": 1, "gates": {"preset": "strict"}}).to_string(),
  )
  .expect("Qwen config");
  let payload = json!({
    "session_id": "s1", "hook_event_name": "PreToolUse",
    "tool_name": "run_shell_command",
    "tool_input": {"command": "git push", "directory": target.sb.path("project")},
  });
  let got = hook.run(Phase::Pre, &payload.to_string(), PRE_TOOL, &[]);
  let output: serde_json::Value =
    serde_json::from_str(&got.result.stdout).expect("Qwen push decision");
  let decision = &output["hookSpecificOutput"];
  assert_eq!(decision["permissionDecision"], "deny", "{got:?}");
  let reason = decision["permissionDecisionReason"].as_str().unwrap();
  assert!(reason.contains("Code review required before push"));
  assert!(reason.contains("feat_example.json"), "{reason}");
}
