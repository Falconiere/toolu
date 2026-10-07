//! Lease admission and reclamation over real isolated resource homes (`resources.test.ts`).

use std::os::unix::process::CommandExt as _;
use std::path::Path;
use std::process::Command;
use std::time::SystemTime;

use toolu_runtime::json::ordered::Ordered;
use toolu_runtime::process::group;

use super::{
  LeaseKind, LeasePatch, LeaseRequest, acquire_lease, epoch_ms, patch_lease,
  reconcile_resource_jobs, release_lease,
};
use crate::resources::lock::write_json_atomic;
use crate::resources::pressure::{ResourceSample, advance_pressure};
use crate::resources::store::{leases, read_resource_state};

fn policy(root: &Path, body: &str) {
  std::fs::create_dir_all(root).unwrap();
  std::fs::write(root.join("policy.json"), body).unwrap();
}

fn agent(key: &str, epic: &str, host: &str) -> LeaseRequest {
  LeaseRequest {
    kind: LeaseKind::Agent,
    key: key.to_owned(),
    state_dir: epic.to_owned(),
    host: Some(host.to_owned()),
    host_cap: None,
    epic_cap: None,
    worktree: None,
  }
}

fn job(key: &str) -> LeaseRequest {
  LeaseRequest {
    kind: LeaseKind::Job,
    host: None,
    ..agent(key, key, "")
  }
}

fn token(lease: &Ordered) -> String {
  match lease.get("token") {
    Some(Ordered::String(token)) => token.clone(),
    _ => panic!("no token in {}", lease.to_text(false)),
  }
}

fn now() -> SystemTime {
  SystemTime::now()
}

#[test]
fn the_machine_agent_limit_is_shared_and_tokens_release_only_their_lease() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path();
  policy(
    root,
    r#"{"maxAgents":3,"maxJobs":1,"hosts":{"claude":1,"codex":2}}"#,
  );
  let first = acquire_lease(root, &agent("first", "epic-a", "claude"), now()).unwrap();
  assert_eq!(
    first.get("stage"),
    Some(&Ordered::String("starting".to_owned()))
  );
  let second = acquire_lease(root, &agent("second", "epic-b", "claude"), now());
  assert_eq!(second, Err("claude capacity exhausted".to_owned()));
  let capped = LeaseRequest {
    epic_cap: Some(2),
    ..agent("codex", "epic-a", "codex")
  };
  let codex = acquire_lease(root, &capped, now()).unwrap();
  let third = LeaseRequest {
    key: "third".to_owned(),
    ..capped
  };
  assert_eq!(
    acquire_lease(root, &third, now()),
    Err("epic capacity exhausted".to_owned())
  );
  assert_eq!(
    acquire_lease(root, &agent("codex", "epic-a", "codex"), now()),
    Err("existing ownership; reconcile before retry".to_owned())
  );
  release_lease(root, "not-a-token").unwrap();
  assert_eq!(leases(&read_resource_state(root).unwrap()).len(), 2);
  release_lease(root, &token(&first)).unwrap();
  release_lease(root, &token(&codex)).unwrap();
  assert_eq!(leases(&read_resource_state(root).unwrap()).len(), 0);
}

#[test]
fn capacity_caps_and_cooldowns_refuse_admission() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path();
  policy(root, r#"{"maxAgents":1,"maxJobs":1}"#);
  let held = acquire_lease(root, &agent("a", "x", "claude"), now()).unwrap();
  assert_eq!(
    acquire_lease(root, &agent("b", "y", "codex"), now()),
    Err("agent capacity exhausted (1)".to_owned())
  );
  release_lease(root, &token(&held)).unwrap();
  let bad_cap = LeaseRequest {
    host_cap: Some(0),
    ..agent("a", "x", "claude")
  };
  assert_eq!(
    acquire_lease(root, &bad_cap, now()),
    Err("invalid host capacity".to_owned())
  );
  let bad_epic = LeaseRequest {
    epic_cap: Some(-1),
    ..agent("a", "x", "claude")
  };
  assert_eq!(
    acquire_lease(root, &bad_epic, now()),
    Err("invalid epic capacity".to_owned())
  );
  let until = epoch_ms(now()) + 60_000.0;
  let cooling = format!(
    r#"{{"version":1,"leases":[],"cooldowns":{{"claude":{{"until":{until},"reason":"rate limit"}}}}}}"#
  );
  std::fs::write(root.join("state.json"), cooling).unwrap();
  assert_eq!(
    acquire_lease(root, &agent("a", "x", "claude"), now()),
    Err("claude is cooling down".to_owned())
  );
}

#[test]
fn a_job_is_blocked_by_its_worktree_agent_stage_and_keeps_the_agent() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path();
  policy(root, r#"{"maxAgents":1,"maxJobs":1}"#);
  let owner = LeaseRequest {
    worktree: Some("/w".to_owned()),
    ..agent("a", "e", "codex")
  };
  let agent_lease = acquire_lease(root, &owner, now()).unwrap();
  let agent_token = token(&agent_lease);
  let worktree_job = LeaseRequest {
    worktree: Some("/w".to_owned()),
    ..job("j")
  };
  let running = acquire_lease(root, &worktree_job, now()).unwrap();
  assert_eq!(
    release_lease(root, &agent_token),
    Err("active jobs prevent agent lease release".to_owned())
  );
  release_lease(root, &token(&running)).unwrap();
  let cleaning = LeasePatch {
    stage: Some("cleaning".to_owned()),
    ..LeasePatch::default()
  };
  patch_lease(root, &agent_token, &cleaning).unwrap();
  assert_eq!(
    acquire_lease(root, &worktree_job, now()),
    Err("worktree job admission blocked by agent stage cleaning".to_owned())
  );
  release_lease(root, &agent_token).unwrap();
}

#[test]
fn patches_need_a_known_token_and_a_positive_group() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path();
  let lease = acquire_lease(root, &job("j"), now()).unwrap();
  let patch = LeasePatch {
    group_pid: Some(42),
    stage: Some("running".to_owned()),
    heartbeat_at: Some("H".to_owned()),
  };
  patch_lease(root, &token(&lease), &patch).unwrap();
  let stored = read_resource_state(root).unwrap();
  let text = leases(&stored)[0].to_text(false);
  assert!(text.contains(r#""stage":"running","createdAt":"#), "{text}");
  assert!(
    text.ends_with(r#""heartbeatAt":"H","groupPid":42}"#),
    "{text}"
  );
  assert_eq!(
    patch_lease(root, "lost", &patch),
    Err("resource lease lost".to_owned())
  );
  let zero = LeasePatch {
    group_pid: Some(0),
    ..LeasePatch::default()
  };
  assert_eq!(
    patch_lease(root, &token(&lease), &zero),
    Err("invalid resource lease patch".to_owned())
  );
}

#[test]
fn held_pressure_refuses_admission_unless_policy_disables_it() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path();
  let at = epoch_ms(now());
  let low = |at| ResourceSample {
    at,
    cpus: 10.0,
    load: 1.0,
    available_bytes: 1e9,
    total_bytes: 32e9,
    steal: None,
    ticks: None,
  };
  let held = advance_pressure(
    Some(&advance_pressure(None, &low(at - 60_000.0)).unwrap()),
    &low(at),
  )
  .unwrap();
  assert!(held.held);
  let state = Ordered::Object(vec![
    ("version".to_owned(), Ordered::Number(1.into())),
    ("leases".to_owned(), Ordered::Array(Vec::new())),
    ("cooldowns".to_owned(), Ordered::Object(Vec::new())),
    ("pressure".to_owned(), held.to_ordered()),
  ]);
  write_json_atomic(&root.join("state.json"), &state).unwrap();
  policy(root, r#"{"maxAgents":1}"#);
  let refused = acquire_lease(root, &agent("a", "e", "claude"), now());
  assert_eq!(
    refused,
    Err("resource hold: sustained CPU/load or memory pressure".to_owned())
  );
  policy(root, r#"{"maxAgents":1,"pressure":false}"#);
  acquire_lease(root, &agent("a", "e", "claude"), now()).unwrap();
  policy(root, r#"{"pressure":"off"}"#);
  assert_eq!(
    acquire_lease(root, &agent("b", "e", "claude"), now()),
    Err("invalid resource pressure policy".to_owned())
  );
}

#[test]
fn a_crashed_owner_keeps_capacity_until_its_group_is_gone() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path();
  policy(root, r#"{"maxJobs":1}"#);
  let mut dead = Command::new("true").spawn().unwrap();
  let dead_pid = dead.id();
  dead.wait().unwrap();
  let mut sleeper = Command::new("sleep")
    .arg("30")
    .process_group(0)
    .spawn()
    .unwrap();
  let lease = format!(
    r#"{{"token":"t","type":"job","key":"k","stateDir":"s","ownerPid":{dead_pid},"groupPid":{},"stage":"running","createdAt":"c","heartbeatAt":"h"}}"#,
    sleeper.id()
  );
  std::fs::write(
    root.join("state.json"),
    format!(r#"{{"version":1,"leases":[{lease}],"cooldowns":{{}}}}"#),
  )
  .unwrap();
  assert_eq!(
    acquire_lease(root, &job("blocked"), now()),
    Err("job capacity exhausted (1)".to_owned())
  );
  assert_eq!(reconcile_resource_jobs(root), Ok(0));
  group::terminate(sleeper.id()).unwrap();
  sleeper.wait().unwrap();
  assert_eq!(reconcile_resource_jobs(root), Ok(1));
  let replacement = acquire_lease(root, &job("replacement"), now()).unwrap();
  release_lease(root, &token(&replacement)).unwrap();
}
