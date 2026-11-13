use std::path::Path;

use serde_json::{Map, Value, json};
use toolu_protocol::decision::Decision;
use toolu_protocol::host::Host;
use toolu_protocol::normalized::NormalizedEvent;
use toolu_runtime::env::Env;
use toolu_runtime::registry::rule::RuleContext;

use super::JOB_RESULTS;
use crate::gate::Gate;
use crate::resources::{binding::bind_worktree, job_results};

#[test]
fn a_read_tool_leaves_a_finished_result_for_the_next_edit_or_shell_call() {
  let dir = tempfile::tempdir().unwrap();
  let root = std::fs::canonicalize(dir.path()).unwrap();
  let init = std::process::Command::new("git")
    .arg("init")
    .arg(&root)
    .output()
    .unwrap();
  assert!(
    init.status.success(),
    "{}",
    String::from_utf8_lossy(&init.stderr)
  );
  let resources = root.join("resources");
  bind_worktree(&resources, &root, "issue-878", "").unwrap();
  let mut record = job_results::create(&resources, "issue-878", &root, "gate").unwrap();
  "done".clone_into(&mut record.state);
  record.exit_code = Some(0);
  job_results::update(&resources, &record).unwrap();
  let event: NormalizedEvent = serde_json::from_value(json!({
    "type": "tool/post", "sessionId": "s", "cwd": root, "projectRoot": root,
    "worktree": root, "toolCallId": "call", "toolName": "Read", "toolInput": {}
  }))
  .unwrap();
  let env = Env::from_pairs([("HOME", root.join("home").to_string_lossy().into_owned())]);
  let raw = Map::<String, Value>::new();
  let ctx = RuleContext {
    host: Host::Claude,
    env: &env,
    config_root: Path::new("/nonexistent"),
    project_root: &root,
    cwd: Some(&root),
    plugin_root: None,
    raw: &raw,
    edit: None,
  };
  assert_eq!(JOB_RESULTS.run(&event, &ctx), Ok(Decision::Allow));
  assert!(
    job_results::get(&resources, &record.id)
      .unwrap()
      .unwrap()
      .delivered_at
      .is_none()
  );
}
