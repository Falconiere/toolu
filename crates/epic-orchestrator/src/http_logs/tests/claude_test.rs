use std::os::unix::fs::symlink;

use serde_json::Value;

use super::claude_logs;
use crate::model::Issue;

#[test]
fn claude_task_discovery_uses_only_regular_files_in_the_workers_slug() {
  let worktree = tempfile::tempdir_in(super::claude_temp()).unwrap();
  let mut issue = Issue::blank("claude-worker", "epic", "/tmp/epic", 0);
  issue.kind = "claude".to_owned();
  issue.stage = "running".to_owned();
  issue.worktree = Some(worktree.path().display().to_string());
  let root = super::claude_root(&issue).unwrap();
  let tasks = root.join("session-1/tasks");
  std::fs::create_dir_all(&tasks).unwrap();
  std::fs::write(tasks.join("task-a.output"), "first line\nsecond line\n").unwrap();
  let outside = worktree.path().join("outside");
  std::fs::write(&outside, "private outside text").unwrap();
  symlink(&outside, tasks.join("other.output")).unwrap();
  let secrets = toolu_runtime::config::secrets::Secrets::none();
  let logs = claude_logs(&issue, &secrets).unwrap();
  assert_eq!(logs.len(), 1);
  assert_eq!(logs[0].label, "first line");
  assert_eq!(logs[0].state, "running");
  assert_eq!(logs[0].size, 23);
  let response = crate::http_logs::detail(
    &logs,
    &logs[0].id,
    "offset=0",
    &toolu_runtime::config::secrets::Secrets::none(),
  );
  let body: Value = serde_json::from_slice(&response.body).unwrap();
  assert_eq!(body["text"], "first line\nsecond line\n");
  issue.kind = "codex".to_owned();
  assert!(claude_logs(&issue, &secrets).unwrap().is_empty());
  std::fs::remove_dir_all(root).unwrap();
}
