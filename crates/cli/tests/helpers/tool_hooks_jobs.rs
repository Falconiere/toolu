//! Finished detached job notices through the real post-tool binary.

use std::path::Path;

use toolu_engine::resources::{binding::bind_worktree, job_results};

use super::{Res, Sandbox, executable, streams};

impl Sandbox {
  fn bound_job(&self, worktree: &Path, label: &str, exit_code: i64) -> Res<job_results::Record> {
    let resources = self.root.join("resources");
    let project = self.root.join("project");
    if !project.join(".git").exists() {
      let status = std::process::Command::new("git")
        .arg("init")
        .arg("-q")
        .arg(&project)
        .status()?;
      assert!(status.success());
    }
    bind_worktree(&resources, &project, "issue-878", "")?;
    let mut record = job_results::create(&resources, "issue-878", worktree, label)?;
    record.state = "done".to_owned();
    record.exit_code = Some(exit_code);
    record.finished_at = Some(record.started_at.clone());
    record.output = "command failed\n".to_owned();
    job_results::update(&resources, &record)?;
    Ok(record)
  }
}

#[test]
fn a_finished_job_is_reported_once_after_a_shell_call() {
  let sb = Sandbox::new().unwrap();
  let project = sb.root.join("project");
  let record = sb.bound_job(&project, "cargo test", 0).unwrap();
  let payload = r#"{"tool_name":"Bash","tool_input":{"command":"ls"},"tool_response":{}}"#;
  let first = sb.hook("post-tools", payload, None).unwrap();
  let doc: serde_json::Value = serde_json::from_slice(&first.stdout).unwrap();
  assert_eq!(
    doc["hookSpecificOutput"]["additionalContext"],
    job_results::line(&record)
  );
  assert_eq!(first.status.code(), Some(0));
  assert_eq!(
    streams(&sb.hook("post-tools", payload, None).unwrap()).0,
    ""
  );
  assert!(
    job_results::get(&sb.root.join("resources"), &record.id)
      .unwrap()
      .unwrap()
      .delivered_at
      .is_some()
  );
}

#[test]
fn another_worktrees_job_is_not_reported_after_an_edit() {
  let sb = Sandbox::new().unwrap();
  let other = sb.root.join("other");
  let record = sb.bound_job(&other, "cargo test", 1).unwrap();
  let payload = r#"{"tool_name":"Edit","tool_input":{"file_path":"test.rs","old_string":"a","new_string":"b"},"tool_response":{}}"#;
  assert_eq!(
    streams(&sb.hook("post-tools", payload, None).unwrap()).0,
    ""
  );
  assert!(
    job_results::get(&sb.root.join("resources"), &record.id)
      .unwrap()
      .unwrap()
      .delivered_at
      .is_none()
  );
}

#[test]
fn a_post_tool_block_still_delivers_the_finished_job() {
  let sb = Sandbox::new().unwrap();
  let project = sb.root.join("project");
  let record = sb.bound_job(&project, "cargo test", 1).unwrap();
  let module = sb.registry("post-tools.d").join("x@t__block.sh");
  executable(
    &module,
    "/bin/sh",
    "printf '{\"decision\":\"block\",\"reason\":\"bad edit\"}\\n'",
  )
  .unwrap();
  let payload = r#"{"tool_name":"Bash","tool_input":{"command":"ls"},"tool_response":{}}"#;
  let blocked = sb.hook("post-tools", payload, None).unwrap();
  let doc: serde_json::Value = serde_json::from_slice(&blocked.stdout).unwrap();
  assert_eq!(doc["decision"], "block");
  assert_eq!(
    doc["reason"],
    format!("bad edit\n\n{}\ncommand failed", job_results::line(&record))
  );
  assert!(
    job_results::get(&sb.root.join("resources"), &record.id)
      .unwrap()
      .unwrap()
      .delivered_at
      .is_some()
  );
  std::fs::remove_file(module).unwrap();
  assert_eq!(
    streams(&sb.hook("post-tools", payload, None).unwrap()).0,
    ""
  );
}

#[test]
fn a_post_tool_exit_two_still_prints_the_finished_job() {
  let sb = Sandbox::new().unwrap();
  let project = sb.root.join("project");
  let record = sb.bound_job(&project, "cargo test", 1).unwrap();
  sb.module("post-tools.d", "x@t__deny.sh", "echo denied >&2; exit 2")
    .unwrap();
  let payload = r#"{"tool_name":"Bash","tool_input":{"command":"ls"},"tool_response":{}}"#;
  let result = sb.hook("post-tools", payload, None).unwrap();
  assert_eq!(result.status.code(), Some(2));
  let stderr = String::from_utf8_lossy(&result.stderr);
  assert!(stderr.contains("denied"), "{stderr}");
  assert!(stderr.contains(&job_results::line(&record)), "{stderr}");
}
