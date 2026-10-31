use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

use super::{Msg, StateContext, handle_fact, on_timeout, publish, request_line};
use crate::model::World;
use crate::paths::Paths;
use crate::server::{Engine, Fault};

#[test]
fn unchanged_turns_do_not_clone_a_large_world() {
  let root = tempfile::tempdir().expect("root");
  let mut engine = large_engine(root.path());
  let shared = Mutex::new(engine.world.clone());
  let changed = Condvar::new();
  let before = issue_key_ptr(&shared, "issue-0001");
  let clone_elapsed = time_clones(&engine.world);
  engine.dirty = true;
  let mut next_tick = Instant::now() + Duration::from_secs(60);
  on_timeout(&mut engine, &mut Vec::new(), &mut next_tick, &()).expect("idle");
  publish(&shared, &changed, &mut engine).expect("idle publish");
  assert_eq!(
    issue_key_ptr(&shared, "issue-0001"),
    before,
    "idle timeout cloned"
  );
  let patch_elapsed = time_clock_publishes(&mut engine, &shared, &changed);
  assert_eq!(
    issue_key_ptr(&shared, "issue-0001"),
    before,
    "clock publishes cloned; patch={patch_elapsed:?} clone={clone_elapsed:?}"
  );
  assert!(shared.lock().expect("lock").generation > 0);
  eprintln!("large world: 20 clones {clone_elapsed:?}; 20 clock publishes {patch_elapsed:?}");
}

#[test]
fn a_stage_edit_clones_once_and_a_repeat_publish_does_not() {
  let root = tempfile::tempdir().expect("root");
  let mut engine = large_engine(root.path());
  let shared = Mutex::new(engine.world.clone());
  let changed = Condvar::new();
  let before = issue_key_ptr(&shared, "issue-0001");
  engine.dirty = true;
  engine
    .world
    .issues
    .get_mut("issue-0001")
    .expect("issue")
    .stage = "running".into();
  publish(&shared, &changed, &mut engine).expect("edit");
  let edited = issue_key_ptr(&shared, "issue-0001");
  assert_ne!(edited, before);
  assert_eq!(
    shared.lock().expect("lock").issues["issue-0001"].stage,
    "running"
  );
  let generation = shared.lock().expect("lock").generation;
  publish(&shared, &changed, &mut engine).expect("repeat");
  assert_eq!(issue_key_ptr(&shared, "issue-0001"), edited);
  assert_eq!(shared.lock().expect("lock").generation, generation);
}

#[test]
fn reader_visible_edits_refresh_one_status_document() {
  let root = tempfile::tempdir().expect("root");
  let (mut engine, paths) = seed_status(root.path());
  let shared = Mutex::new(engine.world.clone());
  let changed = Condvar::new();
  let cache = crate::http_response::OverviewCache::new();
  let secrets = toolu_runtime::config::secrets::Secrets::none();
  let read = |world: &World| overview(&cache, world, &paths, &secrets);
  let _first = read(&shared.lock().expect("lock"));
  let _again = read(&shared.lock().expect("lock"));
  assert_eq!(cache.serializations.load(Ordering::Relaxed), 1);
  let ptr = shared.lock().expect("lock").issues["a"].key.as_ptr();
  engine.world.now_ms = 5_000;
  commit(&mut engine, &shared, &changed);
  assert_eq!(shared.lock().expect("lock").issues["a"].key.as_ptr(), ptr);
  assert_eq!(
    read(&shared.lock().expect("lock"))["attention"][0]["age_ms"],
    4_000
  );
  assert_eq!(cache.serializations.load(Ordering::Relaxed), 2);
  engine.world.issues.get_mut("a").expect("issue").stage = "running".into();
  commit(&mut engine, &shared, &changed);
  assert_eq!(
    read(&shared.lock().expect("lock"))["issues"][0]["stage"],
    "running"
  );
  watch_attention_and_graph(&mut engine, &shared, &changed, &read);
  let built = cache.serializations.load(Ordering::Relaxed);
  let _repeat = read(&shared.lock().expect("lock"));
  assert_eq!(cache.serializations.load(Ordering::Relaxed), built);
  assert!(built > 2);
}

fn watch_attention_and_graph(
  engine: &mut Engine,
  shared: &Mutex<World>,
  changed: &Condvar,
  read: &impl Fn(&World) -> serde_json::Value,
) {
  engine
    .world
    .watches
    .get_mut("pr-a")
    .expect("watch")
    .last_at_ms = Some(42);
  commit(engine, shared, changed);
  assert_eq!(
    read(&shared.lock().expect("lock"))["issues"][0]["github"]["lastCheckAt"],
    42
  );
  engine.world.attention[0].note = "second note".into();
  commit(engine, shared, changed);
  assert_eq!(
    read(&shared.lock().expect("lock"))["attention"][0]["note"],
    "second note"
  );
  engine
    .world
    .epics
    .get_mut("epic-1")
    .expect("epic")
    .waves
    .insert("a".into(), 2);
  commit(engine, shared, changed);
  assert_eq!(
    read(&shared.lock().expect("lock"))["epics"][0]["waves_total"],
    1
  );
}

#[test]
fn cleanup_reply_follows_the_committed_snapshot() {
  let root = tempfile::tempdir().expect("root");
  let mut engine = Engine::open(Paths::at(root.path()), None, Fault::None).expect("engine");
  engine.world.now_ms = 8_000;
  engine.dirty = true;
  let rig = Rig::new(World::new(0));
  let (reply_tx, reply_rx) = mpsc::channel();
  let mut waiters = Vec::new();
  request_line(
    &mut engine,
    &serde_json::json!({"op": "cleanup"}),
    &reply_tx,
    &mut waiters,
    &rig.context(),
  )
  .expect("cleanup");
  let reply = reply_rx.try_recv().expect("reply");
  assert!(reply.get("error").is_none(), "{reply}");
  let published = rig.shared.lock().expect("lock");
  assert_eq!(published.now_ms, engine.world.now_ms);
  assert_eq!(published.generation, engine.world.generation);
}

#[test]
fn resubscribe_publishes_the_pane_before_the_wake_flag() {
  let root = tempfile::tempdir().expect("root");
  let mut engine = Engine::open(Paths::at(root.path()), None, Fault::None).expect("engine");
  let tree = root.path().display().to_string();
  let mut issue = crate::model::Issue::blank("i", "e", &tree, 1_000);
  issue.stage = "running".into();
  issue.worktree = Some(tree.clone());
  engine.world.issues.insert("i".into(), issue);
  let rig = Rig::new(engine.world.clone());
  engine.dirty = true;
  let fact = crate::source::Fact::Event(serde_json::json!({
    "event": "pane.created",
    "cwd": tree,
    "pane_id": "pane-9",
  }));
  let mut running = std::collections::BTreeSet::new();
  handle_fact(&mut engine, fact, &rig.context(), (&mut running, false)).expect("fact");
  assert!(rig.resub.load(Ordering::Relaxed));
  assert_eq!(
    rig.shared.lock().expect("lock").issues["i"].pane.as_deref(),
    Some("pane-9")
  );
  assert!(!engine.resubscribe);
}

fn large_engine(root: &std::path::Path) -> Engine {
  let mut engine = Engine::open(Paths::at(root), None, Fault::None).expect("engine");
  let tree = root.display().to_string();
  for index in 0..400 {
    let key = format!("issue-{index:04}");
    engine.world.issues.insert(
      key.clone(),
      crate::model::Issue::blank(&key, "epic", &tree, 1_000),
    );
  }
  engine
}

fn time_clones(world: &World) -> Duration {
  let started = Instant::now();
  for _ in 0..20 {
    let _cloned = world.clone();
  }
  started.elapsed()
}

fn time_clock_publishes(engine: &mut Engine, shared: &Mutex<World>, changed: &Condvar) -> Duration {
  let started = Instant::now();
  for _ in 0..20 {
    engine.dirty = true;
    engine.world.now_ms = engine.world.now_ms.saturating_add(1);
    publish(shared, changed, engine).expect("clock");
  }
  started.elapsed()
}

fn seed_status(root: &std::path::Path) -> (Engine, Paths) {
  let paths = Paths::at(root);
  let mut engine = Engine::open(paths.clone(), None, Fault::None).expect("engine");
  let tree = root.display().to_string();
  crate::disk::write_value(
    &paths.registry(),
    &serde_json::json!({"version": 1, "epics": [{"key": "epic-1", "state_dir": tree}]}),
  )
  .expect("registry");
  {
    let issue = crate::logic::ensure_issue(&mut engine.world, "a", "epic-1", &tree);
    issue.stage = "queued".into();
    issue.phase = "execution".into();
    issue.phase_at_ms = 1_000;
  }
  engine.world.now_ms = 1_000;
  engine.world.watches.insert(
    "pr-a".into(),
    crate::watch::Watch::new(
      crate::watch::Kind::Pr {
        key: "a".into(),
        repo: "Falconiere/toolu".into(),
        number: 758,
      },
      1_000,
    ),
  );
  crate::attention::raise(&mut engine.world, "stall", "a", "first note");
  engine.world.epics.insert(
    "epic-1".into(),
    crate::epics::Epic::new("epic-1", &tree, 1_000, None),
  );
  (engine, paths)
}

fn overview(
  cache: &crate::http_response::OverviewCache,
  world: &World,
  paths: &Paths,
  secrets: &toolu_runtime::config::secrets::Secrets,
) -> serde_json::Value {
  let response = cache.response(world, paths, secrets);
  serde_json::from_slice(&response.body).expect("overview")
}

fn commit(engine: &mut Engine, shared: &Mutex<World>, changed: &Condvar) {
  engine.dirty = true;
  publish(shared, changed, engine).expect("publish");
}

fn issue_key_ptr(shared: &Mutex<World>, key: &str) -> *const u8 {
  shared.lock().expect("lock").issues[key].key.as_ptr()
}

struct Rig {
  shared: Mutex<World>,
  changed: Condvar,
  resub: AtomicBool,
  interrupt: crate::herdr_interrupt::Interrupt,
  sender: mpsc::Sender<Msg>,
}

impl Rig {
  fn new(world: World) -> Self {
    Self {
      shared: Mutex::new(world),
      changed: Condvar::new(),
      resub: AtomicBool::new(false),
      interrupt: crate::herdr_interrupt::Interrupt::default(),
      sender: mpsc::channel().0,
    }
  }

  fn context(&self) -> StateContext<'_> {
    StateContext {
      protocol: 1,
      sender: &self.sender,
      shared: &self.shared,
      changed: &self.changed,
      resub: &self.resub,
      interrupt: &self.interrupt,
      babysit: &(),
    }
  }
}
