//! Agent migration and worktree fences over real resource homes (`resources.test.ts`).

use std::path::{Path, PathBuf};
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime};

use toolu_runtime::process::{Output, Spec};

use super::rig::{HOSTS, cool, field, home, lease, prepare, request, source};
use super::{
  FenceError, MigrationCaps, active_jobs, cancel_agent_migration, fence_worktree,
  migrate_agent_lease, prepare_agent_migration,
};
use crate::resources::binding::ResourceBinding;
use crate::resources::jobs::run_managed_job;
use crate::resources::leases::{LeaseKind, LeasePatch, acquire_lease, patch_lease, release_lease};
use crate::resources::pressure::{ResourceSample, advance_pressure};
use crate::resources::store::update_resources;

#[test]
fn a_full_or_cooling_destination_leaves_the_source_running() {
  let (dir, root) = home(HOSTS);
  let moving = source(&root, dir.path());
  let occupied = lease(
    &root,
    &request(LeaseKind::Agent, "occupied", Some("codex"), None),
  );
  assert!(
    prepare(&root, &moving, "codex")
      .expect_err("full")
      .contains("codex capacity")
  );
  assert_eq!(field(&root, &moving, "host").as_deref(), Some("claude"));
  assert_eq!(field(&root, &moving, "stage").as_deref(), Some("running"));
  release_lease(&root, &occupied).expect("release");
  cool(&root, "codex", 60_000);
  assert!(
    prepare(&root, &moving, "codex")
      .expect_err("cooling")
      .contains("cooling down")
  );
  cool(&root, "codex", -1);
  prepare(&root, &moving, "codex").expect("an expired cooldown");
  assert_eq!(
    field(&root, &moving, "pendingHost").as_deref(),
    Some("codex")
  );
  assert_eq!(field(&root, &moving, "stage").as_deref(), Some("replacing"));
}

#[test]
fn a_reserved_destination_moves_only_after_old_jobs_exit_and_can_be_cancelled() {
  let (dir, root) = home(HOSTS);
  let moving = source(&root, dir.path());
  let job = lease(
    &root,
    &request(LeaseKind::Job, "old-host-work", None, Some(dir.path())),
  );
  prepare(&root, &moving, "codex").expect("reserve");
  assert_eq!(
    field(&root, &moving, "pendingHost").as_deref(),
    Some("codex")
  );
  assert_eq!(field(&root, &moving, "stage").as_deref(), Some("replacing"));
  let racer = request(LeaseKind::Agent, "racer", Some("codex"), None);
  let refused = acquire_lease(&root, &racer, SystemTime::now()).expect_err("racer");
  assert!(refused.contains("codex capacity"), "{refused}");
  let now = SystemTime::now;
  assert!(
    migrate_agent_lease(&root, &moving, "codex", now())
      .expect_err("jobs")
      .contains("active jobs")
  );
  release_lease(&root, &job).expect("release job");
  migrate_agent_lease(&root, &moving, "codex", now()).expect("migrate");
  assert_eq!(field(&root, &moving, "host").as_deref(), Some("codex"));
  assert_eq!(field(&root, &moving, "pendingHost"), None);
  prepare(&root, &moving, "claude").expect("again");
  cancel_agent_migration(&root, &moving, now()).expect("cancel");
  assert_eq!(field(&root, &moving, "stage").as_deref(), Some("running"));
  assert_eq!(field(&root, &moving, "host").as_deref(), Some("codex"));
}

#[test]
fn migration_refuses_bad_caps_lost_leases_and_a_second_reservation() {
  let (_dir, root) = home(r#"{"maxAgents": 3, "maxJobs": 1}"#);
  let now = SystemTime::now();
  let bad = MigrationCaps {
    host_cap: 0,
    epic_cap: None,
  };
  assert_eq!(
    prepare_agent_migration(&root, "x", "codex", bad, now),
    Err("invalid host capacity".to_owned())
  );
  assert_eq!(
    prepare(&root, "x", ""),
    Err("invalid target host".to_owned())
  );
  let epic = MigrationCaps {
    host_cap: 1,
    epic_cap: Some(0),
  };
  assert_eq!(
    prepare_agent_migration(&root, "x", "codex", epic, now),
    Err("invalid epic capacity".to_owned())
  );
  assert_eq!(
    prepare(&root, "gone", "codex"),
    Err("agent resource lease lost".to_owned())
  );
  let agent = lease(&root, &request(LeaseKind::Agent, "a", Some("claude"), None));
  assert_eq!(
    migrate_agent_lease(&root, &agent, "codex", now),
    Err("agent target capacity is not reserved".to_owned())
  );
  assert_eq!(
    cancel_agent_migration(&root, &agent, now),
    Err("agent migration is not reserved".to_owned())
  );
  prepare(&root, &agent, "codex").expect("reserve");
  prepare(&root, &agent, "codex").expect("same target again");
  assert_eq!(
    prepare(&root, &agent, "cursor"),
    Err("agent migration already reserved".to_owned())
  );
}

struct Fenced {
  _dir: tempfile::TempDir,
  root: PathBuf,
  tree: String,
  agent: String,
  binding: ResourceBinding,
}

fn fenced_agent() -> Fenced {
  let (dir, root) = home(r#"{"maxAgents": 1, "maxJobs": 1}"#);
  let worktree = dir.path().join("wt");
  std::fs::create_dir_all(&worktree).expect("wt");
  let agent = lease(
    &root,
    &request(
      LeaseKind::Agent,
      "issue-376",
      Some("codex"),
      Some(&worktree),
    ),
  );
  let patch = LeasePatch {
    stage: Some("running".into()),
    ..LeasePatch::default()
  };
  patch_lease(&root, &agent, &patch).expect("running");
  let binding = ResourceBinding {
    root: root.clone(),
    key: "issue-376".into(),
    state_dir: "epic".into(),
    worktree: worktree.clone(),
  };
  Fenced {
    _dir: dir,
    tree: worktree.display().to_string(),
    root,
    agent,
    binding,
  }
}

fn managed(binding: &ResourceBinding, argv: &[&str]) -> Result<Output, String> {
  let argv: Vec<String> = argv.iter().map(|arg| (*arg).to_owned()).collect();
  run_managed_job(&argv, binding, &Spec::new(argv.clone()))
}

/// A managed job that runs until `marker` exists.
fn start_job(binding: &ResourceBinding, marker: &Path) -> JoinHandle<Result<Output, String>> {
  let background = binding.clone();
  let wait = format!("while [ ! -e '{}' ]; do sleep 0.05; done", marker.display());
  std::thread::spawn(move || managed(&background, &["sh", "-c", &wait]))
}

#[test]
fn a_live_job_refuses_the_fence_and_the_release() {
  let fenced = fenced_agent();
  let marker = fenced.root.join("job-may-exit");
  let job = start_job(&fenced.binding, &marker);
  for _ in 0..100 {
    if active_jobs(&fenced.root, &fenced.tree).expect("jobs") {
      break;
    }
    std::thread::sleep(Duration::from_millis(10));
  }
  assert!(active_jobs(&fenced.root, &fenced.tree).expect("jobs"));
  assert!(
    release_lease(&fenced.root, &fenced.agent)
      .expect_err("release")
      .contains("active jobs")
  );
  let fence = fence_worktree(&fenced.root, &fenced.agent, &fenced.tree, SystemTime::now());
  assert_eq!(fence, Err(FenceError::JobsActive));
  assert_eq!(
    field(&fenced.root, &fenced.agent, "stage").as_deref(),
    Some("cleanup-incomplete")
  );
  std::fs::write(&marker, "").expect("marker");
  assert_eq!(job.join().expect("join").expect("job").exit_code, 0);
}

#[test]
fn a_fence_blocks_new_jobs_and_refuses_another_worktree() {
  let fenced = fenced_agent();
  fence_worktree(&fenced.root, &fenced.agent, &fenced.tree, SystemTime::now()).expect("fence");
  assert_eq!(
    field(&fenced.root, &fenced.agent, "stage").as_deref(),
    Some("cleaning")
  );
  let marker = fenced.root.join("must-not-start");
  let refused =
    managed(&fenced.binding, &["touch", &marker.display().to_string()]).expect_err("fenced");
  assert!(
    refused.contains("worktree job admission blocked"),
    "{refused}"
  );
  assert!(!marker.exists());
  let elsewhere = fence_worktree(&fenced.root, &fenced.agent, "/elsewhere", SystemTime::now());
  assert_eq!(
    elsewhere.map_err(|err| err.to_string()),
    Err("agent lease worktree does not match cleanup target".to_owned())
  );
  let empty = fence_worktree(&fenced.root, &fenced.agent, "", SystemTime::now());
  assert_eq!(
    empty.map_err(|err| err.to_string()),
    Err("invalid fenced worktree".to_owned())
  );
  release_lease(&fenced.root, &fenced.agent).expect("release");
}

/// A held pressure sample in the resource state.
fn hold_pressure(root: &Path) {
  let at = f64::from(
    u32::try_from(toolu_state::time::epoch_millis(SystemTime::now()) / 1000).expect("secs"),
  ) * 1000.0;
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
    Some(&advance_pressure(None, &low(at - 60_000.0)).expect("first")),
    &low(at),
  )
  .expect("held");
  assert!(held.held);
  update_resources(root, |state| {
    state.set("pressure", held.to_ordered());
    Ok(())
  })
  .expect("pressure");
}

/// The source is still running on claude with nothing reserved.
fn assert_untouched(root: &Path, moving: &str) {
  assert_eq!(field(root, moving, "stage").as_deref(), Some("running"));
  assert_eq!(field(root, moving, "host").as_deref(), Some("claude"));
  assert_eq!(field(root, moving, "pendingHost"), None);
}

#[test]
fn full_machine_or_epic_capacity_and_a_pressure_hold_refuse_the_reservation() {
  let (dir, root) = home(r#"{"maxAgents": 4, "maxJobs": 1}"#);
  let moving = source(&root, dir.path());
  for key in ["first", "second"] {
    let mut sibling = request(LeaseKind::Agent, key, None, None);
    "epic-moving".clone_into(&mut sibling.state_dir);
    lease(&root, &sibling);
  }
  let refused = prepare(&root, &moving, "codex").expect_err("epic");
  assert_eq!(refused, "epic capacity exhausted");
  assert_untouched(&root, &moving);
  std::fs::write(
    root.join("policy.json"),
    r#"{"maxAgents": 2, "maxJobs": 1}"#,
  )
  .expect("policy");
  let refused = prepare(&root, &moving, "codex").expect_err("machine");
  assert_eq!(refused, "agent capacity exhausted (2)");
  assert_untouched(&root, &moving);
  std::fs::write(
    root.join("policy.json"),
    r#"{"maxAgents": 4, "maxJobs": 1}"#,
  )
  .expect("policy");
  hold_pressure(&root);
  let refused = prepare(&root, &moving, "codex").expect_err("pressure");
  assert_eq!(
    refused,
    "resource hold: sustained CPU/load or memory pressure"
  );
  assert_untouched(&root, &moving);
}
