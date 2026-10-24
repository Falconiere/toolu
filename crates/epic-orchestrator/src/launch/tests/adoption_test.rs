use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use serde_json::{Value, json};
use toolu_engine::resources::leases::{
  LeaseKind, LeasePatch, LeaseRequest, acquire_lease, patch_lease,
};
use toolu_engine::resources::lock::acquire_lock;
use toolu_engine::resources::store::{leases, read_resource_state};
use toolu_engine::resources::transitions::{MigrationCaps, prepare_agent_migration};
use toolu_runtime::json::ordered::Ordered;

use super::mirror_running;
use crate::launch::{LaunchOpts, Recovery};
use crate::launch_rig::{Rig, opts};

/// A real resource root holding one agent lease for a worktree, and the
/// record the engine adopted as running.
struct Owned {
  _tmp: tempfile::TempDir,
  root: PathBuf,
  state: PathBuf,
  worktree: String,
  token: String,
}

fn text(lease: &Ordered, key: &str) -> Option<String> {
  match lease.get(key) {
    Some(Ordered::String(value)) => Some(value.clone()),
    _ => None,
  }
}

/// The stage of the lease `token` under `root`.
fn lease_stage(root: &Path, token: &str) -> Option<String> {
  let state = read_resource_state(root).expect("resources");
  leases(&state)
    .iter()
    .find(|lease| text(lease, "token").as_deref() == Some(token))
    .and_then(|lease| text(lease, "stage"))
}

/// A managed job in `worktree`, as `toolu epic job` asks for it.
fn job_in(root: &Path, state: &Path, worktree: &str) -> Result<Ordered, String> {
  let job = LeaseRequest {
    kind: LeaseKind::Job,
    key: "job".into(),
    state_dir: state.display().to_string(),
    host: None,
    host_cap: None,
    epic_cap: None,
    worktree: Some(worktree.to_owned()),
  };
  acquire_lease(root, &job, SystemTime::now())
}

fn owned(stage: &str) -> Owned {
  let tmp = tempfile::tempdir().expect("temp");
  let root = tmp.path().join("resources");
  let state_dir = tmp.path().join("state");
  let worktree = tmp.path().join("wt").display().to_string();
  let agent = LeaseRequest {
    kind: LeaseKind::Agent,
    key: "k-1".into(),
    state_dir: state_dir.display().to_string(),
    host: Some("cursor".into()),
    host_cap: Some(3),
    epic_cap: Some(3),
    worktree: None,
  };
  let lease = acquire_lease(&root, &agent, SystemTime::now()).expect("agent lease");
  let token = text(&lease, "token").expect("token");
  let patch = LeasePatch {
    stage: Some(stage.to_owned()),
    worktree: Some(worktree.clone()),
    ..LeasePatch::default()
  };
  patch_lease(&root, &token, &patch).expect("stage");
  let owned = Owned {
    _tmp: tmp,
    root,
    state: state_dir,
    worktree,
    token,
  };
  owned.record(&json!({"stage": "running", "prompt_state": "submitting", "session_id": null}));
  owned
}

impl Owned {
  /// Write `fields` plus this lease's ownership as the record of k-1.
  fn record(&self, fields: &Value) {
    let mut record = fields.as_object().cloned().unwrap_or_default();
    record.insert("lease_token".into(), json!(self.token));
    record.insert(
      "resource_root".into(),
      json!(self.root.display().to_string()),
    );
    record.insert("worktree".into(), json!(self.worktree));
    let path = self.state.join("issues/k-1.json");
    std::fs::create_dir_all(path.parent().expect("issues")).expect("issues");
    std::fs::write(path, Value::Object(record).to_string()).expect("record");
  }

  fn bytes(&self) -> Vec<u8> {
    std::fs::read(self.root.join("state.json")).expect("resource state")
  }
}

#[test]
fn an_adopted_record_gives_its_unconfirmed_lease_running_and_its_jobs_run() {
  let uncertain = owned("uncertain");
  let job = || job_in(&uncertain.root, &uncertain.state, &uncertain.worktree);
  assert_eq!(
    job().map(|_| ()),
    Err("worktree job admission blocked by agent stage uncertain".to_owned())
  );
  assert_eq!(mirror_running(&uncertain.state, "k-1"), Ok(true));
  assert_eq!(
    lease_stage(&uncertain.root, &uncertain.token).as_deref(),
    Some("running")
  );
  assert!(job().is_ok());
  let starting = owned("starting");
  assert_eq!(mirror_running(&starting.state, "k-1"), Ok(true));
  assert_eq!(
    lease_stage(&starting.root, &starting.token).as_deref(),
    Some("running")
  );
}

#[test]
fn a_launch_in_flight_a_migration_or_a_settled_record_keeps_the_lease() {
  let locked = owned("uncertain");
  let before = locked.bytes();
  let lock = acquire_lock(&locked.state.join("launch-locks/k-1"), Duration::ZERO)
    .expect("lock")
    .expect("free");
  assert_eq!(mirror_running(&locked.state, "k-1"), Ok(false));
  lock.release();
  assert_eq!(locked.bytes(), before);
  let migrating = owned("uncertain");
  let caps = MigrationCaps {
    host_cap: 3,
    epic_cap: Some(3),
  };
  prepare_agent_migration(
    &migrating.root,
    &migrating.token,
    "claude",
    caps,
    SystemTime::now(),
  )
  .expect("pending host");
  let failed = LeasePatch {
    stage: Some("uncertain".to_owned()),
    ..LeasePatch::default()
  };
  patch_lease(&migrating.root, &migrating.token, &failed).expect("failed migration");
  let replacing = owned("replacing");
  let settled = owned("uncertain");
  settled.record(&json!({"stage": "abandoned"}));
  let unowned = owned("uncertain");
  let path = unowned.state.join("issues/k-1.json");
  std::fs::write(&path, json!({"stage": "running"}).to_string()).expect("record");
  for (case, owned) in [
    ("pending host", &migrating),
    ("replacing", &replacing),
    ("abandoned", &settled),
    ("no lease", &unowned),
  ] {
    let before = owned.bytes();
    assert_eq!(mirror_running(&owned.state, "k-1"), Ok(false), "{case}");
    assert_eq!(owned.bytes(), before, "{case}");
  }
}

/// A Cursor launch of comemory-255 whose first prompt stalled before Cursor
/// wrote its chat (#625): the flags it ran with and the record it left.
fn stalled_cursor(rig: &mut Rig) -> (LaunchOpts, Value) {
  rig.checkout("comemory-255");
  rig.herdr.script(
    "agent.prompt",
    crate::herdr_state::Scripted::Error {
      code: "agent_prompt_stalled".into(),
      message: "no working state within 5000 ms".into(),
    },
  );
  let mut flags = opts();
  flags.force = true;
  flags.kind = Some("cursor".into());
  let stalled = rig.launch(&flags).expect_err("stalled");
  assert!(
    stalled.message.contains("agent_prompt_stalled"),
    "{}",
    stalled.message
  );
  (flags, rig.record("comemory-255"))
}

/// The engine recording comemory-255's reporting worker as running.
fn adopt(rig: &Rig) {
  let mut adopted = crate::model::Issue::blank("comemory-255", "epic", &rig.graph.state_dir, 0);
  "running".clone_into(&mut adopted.stage);
  crate::disk::write_issue(&adopted, SystemTime::now()).expect("engine write");
}

/// The chat Cursor kept once the stalled prompt was submitted.
const CHAT: &str = "1f21b969-a107-480c-989c-32f677f90a5f";

/// How many prompts herdr has served.
fn prompts(rig: &Rig) -> usize {
  let calls = rig.herdr.calls();
  calls
    .iter()
    .filter(|(method, _)| method == "agent.prompt")
    .count()
}

/// Write [`CHAT`] as Cursor does, launch `record`'s issue again with
/// `recovery`, and check the record now names the chat, acknowledged.
fn repair(rig: &Rig, mut flags: LaunchOpts, record: &Value, recovery: Recovery) {
  let since = record["session_started_after"].as_u64().expect("since");
  let worktree = record["worktree"].as_str().expect("worktree");
  rig.cursor_chat(worktree, CHAT, since + 1);
  flags.recovery = recovery;
  let launched = rig.launch(&flags);
  launched.unwrap_or_else(|err| panic!("{}", err.message));
  let repaired = rig.record("comemory-255");
  let fields =
    ["stage", "prompt_state", "session_id", "lifecycle_outcome"].map(|key| &repaired[key]);
  let expected = [
    json!("running"),
    json!("acknowledged"),
    json!(CHAT),
    json!("retained"),
  ];
  assert_eq!(fields, expected.each_ref());
}

/// #625 end to end: a Cursor prompt stalls before its chat exists, a human
/// submits it, the worker reports, the engine adopts it, and `--reprompt`
/// records the chat Cursor wrote and prompts the worker again.
#[test]
fn a_stalled_cursor_prompt_runs_jobs_once_adopted_and_reprompt_records_its_chat() {
  let mut rig = Rig::new();
  let (flags, record) = stalled_cursor(&mut rig);
  let unconfirmed = (
    &record["stage"],
    &record["prompt_state"],
    &record["session_id"],
  );
  assert_eq!(
    unconfirmed,
    (&json!("uncertain"), &json!("submitting"), &Value::Null)
  );
  let resources = rig.tmp.path().join("resources");
  let token = record["lease_token"].as_str().expect("lease").to_owned();
  let worktree = record["worktree"].as_str().expect("worktree").to_owned();
  let job = || job_in(&resources, &rig.state(), &worktree);
  assert_eq!(
    job().map(|_| ()),
    Err("worktree job admission blocked by agent stage uncertain".to_owned())
  );
  adopt(&rig);
  assert_eq!(rig.record("comemory-255")["stage"], "running");
  assert_eq!(lease_stage(&resources, &token).as_deref(), Some("running"));
  assert!(job().is_ok());
  let before = prompts(&rig);
  repair(&rig, flags, &record, Recovery::Reprompt);
  assert_eq!(prompts(&rig), before + 1);
  assert_eq!(lease_stage(&resources, &token).as_deref(), Some("running"));
}

/// A plain launch repairs the same record and leaves the working worker
/// unprompted.
#[test]
fn a_plain_launch_records_the_chat_without_prompting_the_worker_again() {
  let mut rig = Rig::new();
  let (flags, record) = stalled_cursor(&mut rig);
  adopt(&rig);
  let before = prompts(&rig);
  repair(&rig, flags, &record, Recovery::Fresh);
  assert_eq!(prompts(&rig), before);
}

#[test]
fn the_engine_write_mirrors_unless_its_own_launch_is_pending() {
  let owned = owned("uncertain");
  let state = owned.state.display().to_string();
  let mut issue = crate::model::Issue::blank("k-1", "epic", &state, 0);
  "running".clone_into(&mut issue.stage);
  let launch = crate::model::fresh(crate::model::Action::Launch, "t1".into());
  issue.pending = Some(launch);
  let before = owned.bytes();
  crate::disk::write_issue(&issue, SystemTime::now()).expect("pending write");
  assert_eq!(owned.bytes(), before);
  issue.pending = None;
  crate::disk::write_issue(&issue, SystemTime::now()).expect("write");
  assert_eq!(
    lease_stage(&owned.root, &owned.token).as_deref(),
    Some("running")
  );
}

/// A mirror that fails is best effort: the engine's record write still
/// stands, through its tracker sighting.
#[test]
fn an_unreadable_resource_state_leaves_the_engine_write_standing() {
  let owned = owned("uncertain");
  std::fs::write(owned.root.join("state.json"), "{").expect("corrupt");
  assert!(mirror_running(&owned.state, "k-1").is_err());
  let state = owned.state.display().to_string();
  let mut issue = crate::model::Issue::blank("k-1", "epic", &state, 0);
  "running".clone_into(&mut issue.stage);
  issue.tracker_seen = crate::model::Seen::Closed;
  crate::disk::write_issue(&issue, SystemTime::now()).expect("the record write stands");
  let record = crate::disk::read_value(&owned.state.join("issues/k-1.json")).expect("record");
  assert_eq!(record["stage"], "running");
  assert!(record.get("tracker_closed_at").is_some(), "{record}");
}
