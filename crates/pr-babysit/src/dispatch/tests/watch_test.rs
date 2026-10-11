use serde_json::json;

use super::*;
use crate::dispatch::sandbox::{FIX, Sandbox, git, sandbox};

fn started(sb: &Sandbox) -> Value {
  let (plan, items) = sb.plan("opencode", Some("probe/m"));
  Dispatcher::new(&sb.options(false))
    .unwrap()
    .start(&sb.start(&plan, &items))
    .unwrap()
}

fn waited(sb: &Sandbox, timeout: u64) -> Value {
  Dispatcher::new(&sb.options(false))
    .unwrap()
    .wait(timeout)
    .unwrap()
}

#[test]
fn without_a_round_wait_answers_none() {
  let sb = sandbox();
  assert_eq!(waited(&sb, 1), json!({"version": 1, "status": "none"}));
}

#[test]
fn a_detached_fixer_that_commits_and_reports_done_finishes_the_round() {
  let sb = sandbox();
  sb.opencode(&format!("sleep 1\n{FIX}"), "done");
  let start = started(&sb);
  assert_eq!(start["status"], "running");
  assert_eq!(start["groups"][0]["status"], "running");
  let done = waited(&sb, 30);
  assert_eq!(done["status"], "done");
  let worktree = done["worktree"].as_str().unwrap();
  let head = git(
    &sb.env,
    std::path::Path::new(worktree),
    &["rev-parse", "HEAD"],
  );
  assert_eq!(done["commits"], json!([head]));
  assert_eq!(sb.state()["fixer"]["groups"][0]["head"], head.as_str());
}

#[test]
fn a_fixer_that_exits_without_a_report_fails_with_its_last_output() {
  let sb = sandbox();
  sb.opencode("echo 'gave up early'", "");
  started(&sb);
  let failed = waited(&sb, 30);
  assert_eq!(failed["status"], "failed");
  assert_eq!(failed["reason"], "no_report");
  let error = failed["groups"][0]["error"].as_str().unwrap();
  assert_eq!(
    error,
    "the fixer exited without a report; last output: gave up early"
  );
}

#[test]
fn a_provider_limit_in_the_fixer_log_cools_the_host() {
  let sb = sandbox();
  let limited = r#"echo '{"type":"error","error":{"name":"APIError","data":{"message":"429 Too Many Requests"}}}'"#;
  sb.opencode(limited, "");
  started(&sb);
  let failed = waited(&sb, 30);
  assert_eq!(failed["reason"], "host_limited");
  assert_eq!(
    sb.state()["hostCooldowns"]["opencode"]["reason"],
    "host_limited"
  );
}

#[test]
fn a_wait_that_runs_out_leaves_the_fixer_running() {
  let sb = sandbox();
  sb.opencode("sleep 30", "done");
  started(&sb);
  let running = waited(&sb, 1);
  assert_eq!(running["status"], "running");
  let group = sb.state()["fixer"]["groups"][0].clone();
  let pid = u32::try_from(group["pid"].as_u64().unwrap()).unwrap();
  assert!(stop_group(pid, group["pidStart"].as_str().unwrap()).unwrap());
}
