use std::time::SystemTime;

use serde_json::{Value, json};
use toolu_engine::resources::binding::bind_worktree;
use toolu_engine::resources::leases::{LeaseKind, LeaseRequest, acquire_lease};
use toolu_runtime::json::ordered::Ordered;

use super::{FinishReport, finish_issue};
use crate::finish_rig::{Layout, agent_lease, layout, leases, patch, read, write};
use crate::launch_rig::Rig;
use crate::support::git;

fn finish(layout: &Layout, abandon: bool) -> Result<FinishReport, String> {
  let rig = Rig::new();
  finish_issue(&layout.state, "k-1", (abandon, false), &mut rig.ctx())
}

#[test]
fn abandon_snapshots_dirty_and_untracked_work_and_records_the_ref() {
  let l = layout();
  std::fs::write(l.wt.join("a.txt"), "two\n").expect("dirty");
  std::fs::write(l.wt.join("new.txt"), "new\n").expect("untracked");
  let report = finish(&l, true).expect("abandon");
  assert_eq!(
    report.warnings,
    ["WARN: no workspace_id on record k-1; skipping worktree remove"]
  );
  let expected = json!({"key": "k-1", "stage": "cleanup-incomplete", "worktree_removed": false, "branch_deleted": false, "wip_ref": "refs/epic-wip/k-1", "leftover_files": [" M a.txt", "?? new.txt"]});
  assert_eq!(report.finished, expected);
  let record = read(&l.record);
  assert_eq!(record["stage"], "cleanup-incomplete");
  assert!(
    record["cleanup_attempt_at"]
      .as_str()
      .is_some_and(|at| at.len() == 20 && at.ends_with('Z'))
  );
  assert!(record.get("finished_at").is_none());
  assert_eq!(git(&l.main, &["show", "refs/epic-wip/k-1:a.txt"]), "two");
  assert_eq!(git(&l.main, &["show", "refs/epic-wip/k-1:new.txt"]), "new");
}

#[test]
fn a_clean_pushed_worktree_records_no_wip_ref() {
  let l = layout();
  let report = finish(&l, true).expect("abandon");
  assert_eq!(report.finished["wip_ref"], Value::Null);
  assert_eq!(report.finished["leftover_files"], json!([]));
}

#[test]
fn without_abandon_a_record_with_no_pr_is_refused_and_left_unchanged() {
  let l = layout();
  let before = std::fs::read_to_string(&l.record).expect("record");
  let refused = finish(&l, false).expect_err("no PR");
  assert_eq!(
    refused,
    "no PR recorded for k-1; use --abandon to tear down anyway"
  );
  assert_eq!(std::fs::read_to_string(&l.record).expect("record"), before);
}

#[test]
fn missing_array_and_malformed_records_fail_without_writing() {
  let tmp = tempfile::tempdir().expect("temp");
  let rig = Rig::new();
  let state = tmp.path().join("state");
  let missing = finish_issue(&state, "k-9", (true, false), &mut rig.ctx()).expect_err("missing");
  assert!(
    missing.starts_with("invalid or missing issue record"),
    "{missing}"
  );
  let issues = state.join("issues");
  std::fs::create_dir_all(&issues).expect("dir");
  std::fs::write(issues.join("k-8.json"), "[1]\n").expect("array");
  let array = finish_issue(&state, "k-8", (true, false), &mut rig.ctx()).expect_err("array");
  assert_eq!(
    array,
    format!(
      "invalid or missing issue record {}",
      issues.join("k-8.json").display()
    )
  );
  std::fs::write(
    issues.join("k-7.json"),
    "{\"key\":\"k-7\",\"worktree\":4}\n",
  )
  .expect("bad");
  let bad = finish_issue(&state, "k-7", (true, false), &mut rig.ctx()).expect_err("bad");
  assert_eq!(
    bad,
    format!(
      "invalid issue record {}: repo",
      issues.join("k-7.json").display()
    )
  );
  assert_eq!(
    std::fs::read_to_string(issues.join("k-8.json")).expect("array"),
    "[1]\n"
  );
}

#[test]
fn a_legacy_bound_worktree_adopts_and_persists_a_cleanup_fence() {
  let l = layout();
  std::fs::create_dir_all(&l.resources).expect("resources");
  std::fs::write(
    l.resources.join("policy.json"),
    r#"{"maxAgents":1,"maxJobs":1}"#,
  )
  .expect("policy");
  let token = agent_lease(&l, "running");
  bind_worktree(&l.resources, &l.wt, "k-1", &l.state.display().to_string()).expect("bind");
  finish(&l, true).expect("abandon");
  let record = read(&l.record);
  assert_eq!(record["resource_root"], json!(l.resources));
  assert_eq!(record["lease_token"], json!(token));
  assert_eq!(record["stage"], "cleanup-incomplete");
  assert_eq!(leases(&l), [(token, "cleaning".to_owned())]);
}

#[test]
fn a_crash_after_a_prepared_removal_is_reconciled() {
  let l = layout();
  let token = agent_lease(&l, "cleaning");
  bind_worktree(&l.resources, &l.wt, "k-1", &l.state.display().to_string()).expect("bind");
  let prepared = toolu_state::time::iso_millis(std::time::SystemTime::now());
  let intent = json!({"version": 1, "workspace_id": "workspace-1", "worktree": l.wt, "prepared_at": prepared, "wip_ref": null, "leftover_files": [], "workload": []});
  patch(
    &l,
    &json!({"stage": "cleanup-incomplete", "workspace_id": "workspace-1", "resource_root": l.resources, "lease_token": token, "worktree_removed": false, "branch_deleted": false, "removal_intent": intent}),
  );
  git(
    &l.main,
    &["worktree", "remove", "--force", &l.wt.display().to_string()],
  );
  let report = finish(&l, true).expect("reconciled");
  assert_eq!(
    (
      report.finished["stage"].clone(),
      report.finished["worktree_removed"].clone()
    ),
    (json!("abandoned"), json!(true))
  );
  assert!(read(&l.record).get("removal_intent").is_none());
  assert_eq!(leases(&l), Vec::new());
  let kept = git(&l.main, &["show-ref", "--verify", "refs/heads/feat/1-x"]);
  assert!(kept.ends_with("refs/heads/feat/1-x"), "{kept}");
}

#[test]
fn an_absent_worktree_without_removal_evidence_is_reconciled_on_abandon() {
  // No agent, pane, or workspace was ever recorded, and the worktree is
  // already gone: failing closed here held the lease forever (#633), while
  // nothing remains to clean. Abandon still keeps the branch.
  let l = layout();
  let token = agent_lease(&l, "cleaning");
  patch(
    &l,
    &json!({"stage": "cleanup-incomplete", "workspace_id": "workspace-1", "resource_root": l.resources, "lease_token": token, "worktree_removed": false, "branch_deleted": false}),
  );
  git(
    &l.main,
    &["worktree", "remove", "--force", &l.wt.display().to_string()],
  );
  let report = finish(&l, true).expect("absent");
  let record = read(&l.record);
  assert_eq!(
    (
      report.finished["stage"].clone(),
      record["worktree_removed"].clone(),
      record["branch_deleted"].clone()
    ),
    (json!("abandoned"), json!(true), json!(false))
  );
  assert!(record.get("removal_intent").is_none());
  assert_eq!(leases(&l), Vec::new());
  assert!(
    git(&l.main, &["show-ref", "--verify", "refs/heads/feat/1-x"]).ends_with("refs/heads/feat/1-x")
  );
}

#[test]
fn an_incomplete_terminal_record_retries_without_releasing() {
  let l = layout();
  let token = agent_lease(&l, "running");
  bind_worktree(&l.resources, &l.wt, "k-1", &l.state.display().to_string()).expect("bind");
  patch(
    &l,
    &json!({"stage": "abandoned", "worktree_removed": false, "branch_deleted": false, "resource_root": l.resources, "lease_token": token}),
  );
  finish(&l, true).expect("retried");
  let record = read(&l.record);
  assert_eq!(
    (record["stage"].clone(), record["worktree_removed"].clone()),
    (json!("cleanup-incomplete"), json!(false))
  );
  assert_eq!(leases(&l), [(token, "cleaning".to_owned())]);
}

/// An `uncertain` record that never made a worktree, owning a fresh lease
/// without a worktree binding; the lease token.
fn unstarted(l: &Layout, checkout: bool) -> String {
  let request = LeaseRequest {
    kind: LeaseKind::Agent,
    key: "k-1".into(),
    state_dir: l.state.display().to_string(),
    host: Some("claude".into()),
    host_cap: None,
    epic_cap: None,
    worktree: None,
  };
  let lease = acquire_lease(&l.resources, &request, SystemTime::now()).expect("lease");
  let token = match lease.get("token") {
    Some(Ordered::String(token)) => token.clone(),
    _ => String::new(),
  };
  let mut body = json!({"key": "k-1", "repo": "o/r", "branch": "feat/1-x", "stage": "uncertain", "resource_root": l.resources, "lease_token": token});
  if checkout {
    body["checkout"] = json!(l.main);
  }
  write(&l.record, &body);
  token
}

#[test]
fn unstarted_abandon_releases_the_lease_and_marks_the_record_abandoned() {
  for checkout in [false, true] {
    let l = layout();
    unstarted(&l, checkout);
    let report = finish(&l, true).expect("abandon");
    assert_eq!(report.finished["stage"], "abandoned");
    assert_eq!(report.finished["worktree_removed"], true);
    let record = read(&l.record);
    assert_eq!(record["stage"], "abandoned");
    assert_eq!(record["worktree_removed"], true);
    assert!(record["finished_at"].is_string());
    assert_eq!(leases(&l).len(), 0);
  }
}

#[test]
fn unstarted_abandon_twice_succeeds_both_times() {
  let l = layout();
  unstarted(&l, false);
  finish(&l, true).expect("first");
  let report = finish(&l, true).expect("second");
  assert_eq!(report.finished["stage"], "abandoned");
  assert_eq!(read(&l.record)["stage"], "abandoned");
}

#[test]
fn unstarted_dry_run_plans_a_release_and_writes_nothing() {
  let l = layout();
  let token = unstarted(&l, false);
  let before = std::fs::read_to_string(&l.record).expect("record");
  let rig = Rig::new();
  let report = finish_issue(&l.state, "k-1", (true, true), &mut rig.ctx()).expect("plan");
  assert_eq!(
    report.finished["actions"],
    json!([format!("release lease {token}")])
  );
  assert_eq!(std::fs::read_to_string(&l.record).expect("record"), before);
  assert_eq!(leases(&l).len(), 1);
}

#[test]
fn unstarted_without_abandon_or_with_a_worktree_only_is_refused_unchanged() {
  let l = layout();
  unstarted(&l, false);
  let before = std::fs::read_to_string(&l.record).expect("record");
  let refused = finish(&l, false).expect_err("no PR");
  assert_eq!(
    refused,
    "no PR recorded for k-1; use --abandon to tear down anyway"
  );
  assert_eq!(std::fs::read_to_string(&l.record).expect("record"), before);
  write(
    &l.record,
    &json!({"key": "k-1", "repo": "o/r", "branch": "b", "worktree": l.wt, "stage": "uncertain"}),
  );
  let refused = finish(&l, true).expect_err("no checkout");
  assert!(refused.ends_with(": checkout"), "{refused}");
}
