use std::path::Path;

use serde_json::json;

use super::{Owner, Reserve, launch_reservation, persist_attempt, with_launch_ownership};
use crate::hosts::HostKind;
use crate::launch::record::Record;
use crate::launch_rig::Rig;

fn reserve<'a>(rig: &'a Rig, state: &'a Path, host: HostKind, replace: bool) -> Reserve<'a> {
  Reserve {
    state,
    key: "issue-1",
    host,
    max_parallel: 3,
    replace,
    env: &rig.env,
  }
}

#[test]
fn invalid_saved_pools_never_fall_back_to_the_default_capacity() {
  let rig = Rig::new();
  let state = rig.state();
  std::fs::create_dir_all(&state).expect("state");
  for pool in [
    "null",
    "[]",
    r#"[{"kind":"codex","cap":0}]"#,
    r#"[{"kind":"codex","cap":1},{"kind":"codex","cap":2}]"#,
    "{",
  ] {
    std::fs::write(state.join("pool.json"), pool).expect("pool");
    let mut ctx = rig.ctx();
    assert!(
      launch_reservation(
        &reserve(&rig, &state, HostKind::Codex, false),
        &mut Record::new(),
        &mut ctx.herdr
      )
      .is_err(),
      "{pool}"
    );
    assert_eq!(rig.leases(), 0);
  }
  std::fs::write(state.join("pool.json"), r#"[{"kind":"claude","cap":1}]"#).expect("pool");
  let mut ctx = rig.ctx();
  let gone = launch_reservation(
    &reserve(&rig, &state, HostKind::Codex, false),
    &mut Record::new(),
    &mut ctx.herdr,
  )
  .expect_err("gone");
  assert_eq!(
    gone.message,
    "codex is no longer in the host pool; reroute first"
  );
}

#[test]
fn a_saved_lease_is_reused_only_by_its_owner() {
  let rig = Rig::new();
  let state = rig.state();
  let mut ctx = rig.ctx();
  let mut record = Record::new();
  let owner = launch_reservation(
    &reserve(&rig, &state, HostKind::Claude, false),
    &mut record,
    &mut ctx.herdr,
  )
  .expect("first");
  persist_attempt(&state, "issue-1", &owner, &mut record, "starting").expect("persist");
  assert_eq!(rig.leases(), 1);
  let again = launch_reservation(
    &reserve(&rig, &state, HostKind::Claude, false),
    &mut record,
    &mut ctx.herdr,
  )
  .expect("again");
  assert_eq!(again, owner);
  let mut stranger = Record::new();
  let foreign = launch_reservation(
    &reserve(&rig, &state, HostKind::Claude, false),
    &mut stranger,
    &mut ctx.herdr,
  )
  .expect_err("foreign");
  assert_eq!(
    foreign.message,
    "existing launch ownership differs; finish/reconcile it before retry"
  );
  let unverified = launch_reservation(
    &reserve(&rig, &state, HostKind::Codex, true),
    &mut record,
    &mut ctx.herdr,
  )
  .expect_err("unverified");
  assert_eq!(
    unverified.message,
    "replacement requires verified ownership and explicit --replace"
  );
  let saved = rig.record("issue-1");
  assert_eq!(
    (saved["stage"].clone(), saved["attempt_id"].clone()),
    (json!("starting"), json!(owner.token))
  );
}

#[test]
fn a_replacement_reserves_the_destination_then_moves_the_lease() {
  let rig = Rig::new();
  let state = rig.state();
  let worktree = rig.tmp.path().join("wt");
  std::fs::create_dir_all(&worktree).expect("worktree");
  let workspace = rig.herdr.add_repo(&worktree);
  let pane = format!("{workspace}:p1");
  let mut ctx = rig.ctx();
  let mut record = Record::new();
  let owner: Owner = launch_reservation(
    &reserve(&rig, &state, HostKind::Claude, false),
    &mut record,
    &mut ctx.herdr,
  )
  .expect("lease");
  record.insert("pane_id".into(), json!(pane));
  record.insert(
    "worktree".into(),
    json!(
      std::fs::canonicalize(&worktree)
        .expect("real")
        .display()
        .to_string()
    ),
  );
  persist_attempt(&state, "issue-1", &owner, &mut record, "starting").expect("persist");
  let moved = launch_reservation(
    &reserve(&rig, &state, HostKind::Codex, true),
    &mut record,
    &mut ctx.herdr,
  )
  .expect("moved");
  assert_eq!(moved, owner);
  let resources =
    toolu_engine::resources::store::read_resource_state(&owner.root).expect("resources");
  let lease = toolu_engine::resources::store::leases(&resources)
    .first()
    .cloned()
    .expect("lease");
  assert_eq!(
    lease.get("host").map(|host| host.to_text(false)),
    Some("\"codex\"".to_owned())
  );
  assert_eq!(rig.record("issue-1")["stage"], "replacing");
}

#[test]
fn one_launch_at_a_time_per_issue() {
  let tmp = tempfile::tempdir().expect("temp");
  let inner = with_launch_ownership(tmp.path(), "k-1", || {
    with_launch_ownership(tmp.path(), "k-1", || Ok(1))
  });
  assert_eq!(
    inner.map_err(|err| err.message),
    Err("launch already in progress for k-1".to_owned())
  );
  assert_eq!(
    with_launch_ownership(tmp.path(), "k-1", || Ok(2)).map_err(|err| err.message),
    Ok(2)
  );
}

#[test]
fn a_lease_whose_owner_and_agent_are_gone_no_longer_blocks_a_launch() {
  let rig = Rig::new();
  let state = rig.state();
  let resources = rig.tmp.path().join("resources");
  let mut child = std::process::Command::new("true").spawn().expect("spawn");
  let dead = child.id();
  child.wait().expect("wait");
  std::fs::create_dir_all(&resources).expect("resources");
  std::fs::write(resources.join("policy.json"), r#"{"maxAgents":1}"#).expect("policy");
  let leftover = format!(
    r#"{{"token":"old","type":"agent","key":"issue-0","stateDir":"s","host":"claude","pane":"pane-0","ownerPid":{dead},"stage":"running","createdAt":"c","heartbeatAt":"h"}}"#
  );
  std::fs::write(
    resources.join("state.json"),
    format!(r#"{{"version":1,"leases":[{leftover}],"cooldowns":{{}}}}"#),
  )
  .expect("state");
  let mut ctx = rig.ctx();
  ctx.herdr.client.pin(toolu_mux::Backend::Herdr);
  launch_reservation(
    &reserve(&rig, &state, HostKind::Claude, false),
    &mut Record::new(),
    &mut ctx.herdr,
  )
  .expect("the leftover lease is reclaimed");
  assert_eq!(rig.leases(), 1);
}

#[test]
fn a_same_host_replacement_at_the_agent_cap_keeps_its_own_slot() {
  let rig = Rig::new();
  let state = rig.state();
  let resources = rig.tmp.path().join("resources");
  std::fs::create_dir_all(&resources).expect("resources");
  std::fs::write(resources.join("policy.json"), r#"{"maxAgents":1}"#).expect("policy");
  let worktree = rig.tmp.path().join("wt");
  std::fs::create_dir_all(&worktree).expect("worktree");
  let workspace = rig.herdr.add_repo(&worktree);
  let mut ctx = rig.ctx();
  let mut record = Record::new();
  let owner = launch_reservation(
    &reserve(&rig, &state, HostKind::Claude, false),
    &mut record,
    &mut ctx.herdr,
  )
  .expect("lease");
  record.insert("pane_id".into(), json!(format!("{workspace}:p1")));
  let real = std::fs::canonicalize(&worktree).expect("real");
  record.insert("worktree".into(), json!(real.display().to_string()));
  persist_attempt(&state, "issue-1", &owner, &mut record, "running").expect("persist");
  let moved = launch_reservation(
    &reserve(&rig, &state, HostKind::Claude, true),
    &mut record,
    &mut ctx.herdr,
  )
  .map_err(|err| err.message);
  assert_eq!(moved, Ok(owner));
  assert_eq!(rig.leases(), 1);
}

#[test]
fn reclaim_only_treats_matching_backend_panes_as_absent() {
  assert!(super::pane_matches_backend(
    Some("hc:t1"),
    toolu_mux::Backend::HerdrApp
  ));
  assert!(!super::pane_matches_backend(
    Some("%1"),
    toolu_mux::Backend::HerdrApp
  ));
  assert!(!super::pane_matches_backend(
    Some("hc:t1"),
    toolu_mux::Backend::Herdr
  ));
  assert!(super::pane_matches_backend(
    Some("%1"),
    toolu_mux::Backend::Tmux
  ));
  assert!(!super::pane_matches_backend(None, toolu_mux::Backend::Tmux));
}
