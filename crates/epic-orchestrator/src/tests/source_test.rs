use std::time::{Duration, SystemTime};

use toolu_runtime::env::Env;

use super::{Pace, apply, session};
use crate::journal;
use crate::paths::Paths;
use crate::server::{Engine, Fault};

use crate::source_fix::{fixture, serve_peer};

#[test]
fn herdr_exit() {
  let fix = fixture(true).expect("fixture");
  let peer = serve_peer(fix.listener, fix.script);
  let started = std::time::Instant::now();
  let engine = drive(&fix.engine_paths, &fix.env);
  assert!(started.elapsed() < Duration::from_secs(2));
  let _ = peer;
  let text = journal_text(&engine.paths);
  assert!(text.contains("pane_closed"), "{text}");
  assert!(!text.contains("agent list"), "{text}");
  assert!(!text.contains("agent.list"), "{text}");
  assert_eq!(engine.world.issues["a"].launches, 1);
}

fn drive(paths: &Paths, env: &Env) -> Engine {
  let mut engine = Engine::open(paths.clone(), None, Fault::None).expect("open");
  let world = engine.world.clone();
  session(env, &world, &pace(), |fact| apply(&mut engine, env, fact)).expect("session");
  engine
}

fn pace() -> Pace {
  Pace {
    idle: Some(Duration::from_millis(200)),
    return_on_idle: true,
    stop: None,
    resubscribe: None,
  }
}

fn journal_text(paths: &Paths) -> String {
  journal::tail(&paths.journal_dir(), SystemTime::now())
    .unwrap_or_default()
    .iter()
    .map(|row| format!("{} {} {}", row.name, row.note, row.kind))
    .collect::<Vec<_>>()
    .join("\n")
}

#[test]
fn herdr_restart() {
  let mut fix = fixture(true).expect("fixture");
  fix.script.event = None;
  fix.script.drop_after_subscribe = true;
  let _peer = serve_peer(fix.listener, fix.script);
  let engine = drive(&fix.engine_paths, &fix.env);
  assert_eq!(engine.world.issues["a"].launches, 0);
  assert!(!journal_text(&engine.paths).contains("herdr-error"));
}

#[test]
fn herdr_snapshot_gone() {
  let mut fix = fixture(true).expect("fixture");
  fix.script.event = None;
  fix.script.drop_after_subscribe = true;
  fix.script.empty_after_first = true;
  let _peer = serve_peer(fix.listener, fix.script);
  let engine = drive(&fix.engine_paths, &fix.env);
  assert_eq!(engine.world.issues["a"].launches, 1);
}

#[test]
fn herdr_idle() {
  let mut fix = fixture(true).expect("fixture");
  let log = fix.tmp.path().join("methods.log");
  fix.script.event = None;
  fix.script.log = Some(log.clone());
  let _peer = serve_peer(fix.listener, fix.script);
  let mut engine = drive(&fix.engine_paths, &fix.env);
  let before = std::fs::read_to_string(&log).unwrap_or_default();
  let status = std::path::Path::new(&engine.world.issues["a"].state_dir).join("status/a.json");
  let stamp = std::fs::metadata(&status)
    .expect("status")
    .modified()
    .expect("mtime");
  engine.tick(&()).expect("tick");
  assert_eq!(std::fs::read_to_string(&log).unwrap_or_default(), before);
  assert_eq!(
    std::fs::metadata(&status)
      .expect("status")
      .modified()
      .expect("mtime"),
    stamp
  );
}

#[test]
fn herdr_outage() {
  let fix = fixture(true).expect("fixture");
  let missing = fix.tmp.path().join("missing.sock");
  let env = Env::from_pairs([
    ("HOME", "/tmp".to_owned()),
    ("HERDR_SOCKET_PATH", missing.display().to_string()),
  ]);
  let mut engine = Engine::open(fix.engine_paths, None, Fault::None).expect("open");
  let world = engine.world.clone();
  session(&env, &world, &pace(), |fact| apply(&mut engine, &env, fact)).expect("outage");
  assert_eq!(engine.world.herdr_retry_at_ms, engine.world.now_ms + 30_000);
  assert_eq!(herdr_errors(&engine.paths), 1);
  let world = engine.world.clone();
  session(&env, &world, &pace(), |fact| apply(&mut engine, &env, fact)).expect("quiet");
  assert_eq!(herdr_errors(&engine.paths), 1);
  engine.world.herdr_failures = 0;
  engine.world.herdr_retry_at_ms = 0;
  if let Some(issue) = engine.world.issues.get_mut("a") {
    issue.stage.clear();
  }
  let world = engine.world.clone();
  session(&env, &world, &pace(), |fact| apply(&mut engine, &env, fact)).expect("idle issue");
  assert_eq!(herdr_errors(&engine.paths), 1);
}

fn herdr_errors(paths: &Paths) -> usize {
  journal_text(paths).matches("herdr-error").count()
}

const IDLE: &str = "{\"event\":\"pane_agent_status_changed\",\"data\":{\"type\":\"pane.agent_status_changed\",\"pane_id\":\"w1:p1\",\"agent_status\":\"idle\"}}\n{\"event\":\"pane_agent_status_changed\",\"data\":{\"type\":\"pane.agent_status_changed\",\"pane_id\":\"w1:p1\",\"agent_status\":\"idle\"}}";

#[test]
fn herdr_limit() {
  let mut fix = fixture(true).expect("fixture");
  let log = fix.tmp.path().join("methods.log");
  fix.script.event = Some(IDLE.to_owned());
  fix.script.read = Some("ok\nusage limit reached".to_owned());
  fix.script.log = Some(log.clone());
  let _peer = serve_peer(fix.listener, fix.script);
  let mut engine = drive(&fix.engine_paths, &fix.env);
  let hosts = std::path::Path::new(&engine.world.issues["a"].state_dir).join("hosts.json");
  let host = std::fs::read_to_string(&hosts).expect("hosts");
  assert!(host.contains("claude"), "{host}");
  let state = std::fs::read_to_string(engine.paths.root.join("state.json")).expect("state");
  assert!(state.contains("usage limit reached"), "{state}");
  let reads = std::fs::read_to_string(&log)
    .unwrap_or_default()
    .matches("agent.read")
    .count();
  assert_eq!(reads, 1);
  engine
    .report(&crate::model::Report {
      key: "a".to_owned(),
      epic: "one".to_owned(),
      state_dir: engine.world.issues["a"].state_dir.clone(),
      phase: "failed".to_owned(),
      pr: None,
      note: "rate-limited: quota exceeded".to_owned(),
    })
    .expect("report");
  let reads = std::fs::read_to_string(&log)
    .unwrap_or_default()
    .matches("agent.read")
    .count();
  assert_eq!(reads, 1);
  let before = journal_text(&engine.paths);
  if let Some(issue) = engine.world.issues.get_mut("a") {
    issue.phase = "ready".to_owned();
  }
  crate::limit::on_status(&mut engine, "a", "idle", "usage limit reached").expect("parked");
  assert_eq!(journal_text(&engine.paths), before);
}

#[test]
fn herdr_gone_checkpoint() {
  let fix = fixture(true).expect("fixture");
  let tree = fix.tmp.path().join("tree");
  git_dirty(&tree);
  let epic = fix.tmp.path().join("epic");
  std::fs::write(
    epic.join("issues").join("a.json"),
    format!(
      r#"{{"stage":"running","worktree":"{}","kind":"claude"}}"#,
      tree.display()
    ),
  )
  .expect("issue");
  let _peer = serve_peer(fix.listener, fix.script);
  let mut engine = drive(&fix.engine_paths, &fix.env);
  assert_eq!(engine.world.issues["a"].launches, 1);
  let reff = std::process::Command::new("git")
    .args([
      "-C",
      &tree.display().to_string(),
      "rev-parse",
      "refs/epic-wip/a",
    ])
    .output()
    .expect("ref");
  assert!(
    reff.status.success(),
    "{}",
    String::from_utf8_lossy(&reff.stderr)
  );
  crate::schedule::note_event(&mut engine.world, "a", "gone");
  crate::schedule::note_event(&mut engine.world, "a", "gone");
  assert!(journal_has_limit(&mut engine));
  if let Some(issue) = engine.world.issues.get_mut("a") {
    issue.stage = "idle".to_owned();
  }
  let launches = engine.world.issues["a"].launches;
  crate::schedule::map_pane_closed(&mut engine.world, "w1:p1");
  assert_eq!(engine.world.issues["a"].launches, launches);
}

fn journal_has_limit(engine: &mut Engine) -> bool {
  engine.flush().expect("flush");
  journal_text(&engine.paths).contains("relaunch-limit")
}

fn git_dirty(tree: &std::path::Path) {
  std::fs::create_dir_all(tree).expect("tree");
  let git = |args: &[&str]| {
    let status = std::process::Command::new("git")
      .args(args)
      .current_dir(tree)
      .env("GIT_AUTHOR_NAME", "epic")
      .env("GIT_AUTHOR_EMAIL", "epic@localhost")
      .env("GIT_COMMITTER_NAME", "epic")
      .env("GIT_COMMITTER_EMAIL", "epic@localhost")
      .status()
      .expect("git");
    assert!(status.success(), "{args:?}");
  };
  git(&["init", "-q"]);
  std::fs::write(tree.join("keep"), "keep\n").expect("keep");
  git(&["add", "keep"]);
  git(&["commit", "-q", "-m", "init"]);
  std::fs::write(tree.join("dirty"), "dirty\n").expect("dirty");
}
