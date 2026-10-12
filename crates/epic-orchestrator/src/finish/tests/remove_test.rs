use serde_json::json;
use toolu_engine::resources::binding::bind_worktree;
use toolu_http_test_support::Fixture;

use crate::finish::finish_issue;
use crate::finish_rig::{Layout, agent_lease, layout, leases, patch, read, write};
use crate::herdr::Call;
use crate::launch_rig::Rig;
use crate::support::{gh, git, ok};

/// The layout's worktree open in herdr with a live Claude agent and a merged
/// or open PR 7; the GitHub fixture.
fn live(l: &Layout, rig: &Rig, merged: bool) -> Fixture {
  let workspace = rig.herdr.state().open(&l.main, &l.wt, "k-1", true);
  let pane = format!("{workspace}:p1");
  let start = Call::Start {
    name: "k-1".into(),
    kind: "claude".into(),
    pane_id: pane.clone(),
    args: Vec::new(),
    timeout_ms: None,
  };
  rig.ctx().herdr.call(&start).expect("agent");
  let token = agent_lease(l, "running");
  bind_worktree(&l.resources, &l.wt, "k-1", &l.state.display().to_string()).expect("bind");
  patch(
    l,
    &json!({"workspace_id": workspace, "pane_id": pane, "agent": "k-1", "kind": "claude", "resource_root": l.resources, "lease_token": token}),
  );
  write(
    &l.state.join("status").join("k-1.json"),
    &json!({"phase": "ready", "pr": 7}),
  );
  let fixture = Fixture::start().expect("fixture");
  let state = if merged { "closed" } else { "open" };
  ok(
    &fixture,
    "/repos/o/r/pulls/7",
    &json!({"merged": merged, "state": state}),
  );
  fixture
}

#[test]
fn a_merged_issue_is_torn_down_through_herdr_and_its_lease_released() {
  let (l, rig) = (layout(), Rig::new());
  let github = live(&l, &rig, true);
  let client = gh(&github);
  let mut ctx = rig.ctx();
  ctx.gh = Ok(&client);
  let report = finish_issue(&l.state, "k-1", (false, false), &mut ctx).expect("finish");
  assert_eq!(
    report.finished,
    json!({"key": "k-1", "stage": "merged", "worktree_removed": true, "branch_deleted": true, "wip_ref": null, "leftover_files": []})
  );
  assert!(!l.wt.exists());
  assert_eq!(leases(&l), Vec::new());
  let record = read(&l.record);
  assert!(record.get("removal_intent").is_none() && record["finished_at"].is_string());
  let removes: Vec<_> = rig
    .herdr
    .calls()
    .into_iter()
    .filter(|(method, _)| method == "worktree.remove")
    .collect();
  assert_eq!(
    removes.first().map(|(_, params)| params["force"].clone()),
    Some(json!(true))
  );
  assert_eq!(rig.herdr.state().agents.len(), 0);
}

#[test]
fn an_open_pr_and_live_workload_refuse_and_keep_the_lease() {
  let (l, rig) = (layout(), Rig::new());
  let github = live(&l, &rig, false);
  let client = gh(&github);
  let mut ctx = rig.ctx();
  ctx.gh = Ok(&client);
  let open = finish_issue(&l.state, "k-1", (false, false), &mut ctx).expect_err("open");
  assert_eq!(open, "PR o/r#7 is OPEN, not MERGED");
  // Its own group, as a worker's job has: in the test runner's group, any
  // exiting process of a parallel test would make ownership unknowable.
  let mut sleeper = std::os::unix::process::CommandExt::process_group(
    std::process::Command::new("sleep")
      .arg("30")
      .current_dir(&l.wt),
    0,
  )
  .spawn()
  .expect("sleep");
  let busy = finish_issue(&l.state, "k-1", (true, false), &mut ctx);
  let _killed = sleeper.kill();
  let _reaped = sleeper.wait();
  assert_eq!(busy, Err("agent k-1 shutdown incomplete".to_owned()));
  let record = read(&l.record);
  assert_eq!(
    (record["stage"].clone(), record["cleanup_error"].clone()),
    (
      json!("cleanup-incomplete"),
      json!("agent k-1 shutdown incomplete")
    )
  );
  assert_eq!(leases(&l).len(), 1);
}

#[test]
fn a_prepared_removal_must_match_and_leave_no_registration() {
  let (l, rig) = (layout(), Rig::new());
  let token = agent_lease(&l, "cleaning");
  let prepared = toolu_state::time::iso_millis(std::time::SystemTime::now());
  let intent = |workspace: &str| json!({"version": 1, "workspace_id": workspace, "worktree": l.wt, "prepared_at": prepared, "wip_ref": null, "leftover_files": [], "workload": []});
  patch(
    &l,
    &json!({"stage": "cleanup-incomplete", "workspace_id": "w-1", "resource_root": l.resources, "lease_token": token, "removal_intent": intent("w-2")}),
  );
  std::fs::remove_dir_all(&l.wt).expect("rm");
  let other = finish_issue(&l.state, "k-1", (true, false), &mut rig.ctx()).expect_err("mismatch");
  assert_eq!(
    other,
    "removal intent does not match recorded workspace ownership"
  );
  patch(&l, &json!({"removal_intent": intent("w-1")}));
  let registered =
    finish_issue(&l.state, "k-1", (true, false), &mut rig.ctx()).expect_err("registered");
  assert_eq!(registered, "worktree path is absent but remains registered");
  git(&l.main, &["worktree", "prune"]);
  let done = finish_issue(&l.state, "k-1", (true, false), &mut rig.ctx()).expect("reconciled");
  assert_eq!(done.finished["stage"], "abandoned");
}

#[test]
fn a_dry_run_plans_the_teardown_and_writes_nothing() {
  let (l, rig) = (layout(), Rig::new());
  patch(&l, &json!({"workspace_id": "w-1", "agent": "k-1"}));
  let before = std::fs::read_to_string(&l.record).expect("record");
  let report = finish_issue(&l.state, "k-1", (true, true), &mut rig.ctx()).expect("dry");
  assert_eq!(
    report.finished["actions"],
    json!([
      "herdr agent prompt k-1 /exit",
      format!("snapshot {} at refs/epic-wip/k-1", l.wt.display()),
      "herdr worktree remove --workspace w-1"
    ])
  );
  assert_eq!(std::fs::read_to_string(&l.record).expect("record"), before);
  assert!(rig.herdr.calls().is_empty() && l.wt.exists());
}
