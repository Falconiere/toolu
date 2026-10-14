use serde_json::json;

use crate::dispatch::dispatcher::Dispatcher;
use crate::dispatch::sandbox::{FIX, Sandbox, git, sandbox};

fn start(sb: &Sandbox) -> serde_json::Value {
  let (plan, items) = sb.plan("claude", Some("opus"));
  Dispatcher::new(&sb.options(false))
    .unwrap()
    .start(&sb.start(&plan, &items))
    .unwrap()
}

#[test]
fn a_herdr_fixer_round_runs_from_start_through_wait_to_cleanup() {
  let sb = sandbox();
  sb.herdr(r#"{"result":{"agent":{"name":"pb"}}}"#, FIX);
  let started = start(&sb);
  assert_eq!(started["status"], "running");
  let worktree = sb.dir.path().join("herdr-wt");
  let recorded = sb.state()["herdrWorktree"].clone();
  assert_eq!(
    (&recorded["workspaceId"], &recorded["paneId"]),
    (&json!("w1"), &json!("p1"))
  );
  let brief = std::fs::read_to_string(sb.state()["fixer"]["groups"][0]["brief"].as_str().unwrap());
  assert!(brief.unwrap().contains("babysit fixer-report"));
  let done = Dispatcher::new(&sb.options(false))
    .unwrap()
    .wait(30)
    .unwrap();
  assert_eq!(done["status"], "done");
  assert_eq!(
    done["commits"],
    json!([git(&sb.env, &worktree, &["rev-parse", "HEAD"])])
  );
  let cleaned = Dispatcher::new(&sb.options(false))
    .unwrap()
    .cleanup()
    .unwrap();
  assert_eq!(
    (&cleaned["worktreeRemoved"], &cleaned["branchDeleted"]),
    (&json!(true), &json!(false))
  );
  assert!(!worktree.exists());
}

#[test]
fn an_agent_that_never_gets_ready_fails_the_group_with_herdr_s_reason() {
  let sb = sandbox();
  sb.herdr(
    r#"{"error":{"code":"agent_not_ready","message":"pb did not get ready"}}"#,
    FIX,
  );
  let failed = start(&sb);
  assert_eq!(failed["reason"], "agent_start_failed");
  let error = failed["groups"][0]["error"].as_str().unwrap();
  assert!(
    error.starts_with("herdr agent start: agent_not_ready: pb did not get ready (the blocked")
  );
}

#[test]
fn a_host_cli_missing_from_path_fails_the_group_before_herdr_starts_it() {
  let sb = sandbox();
  sb.herdr(r#"{"result":{}}"#, FIX);
  std::fs::remove_file(sb.dir.path().join("bin/claude")).unwrap();
  let failed = start(&sb);
  assert_eq!(failed["groups"][0]["error"], "claude is not on PATH");
}
