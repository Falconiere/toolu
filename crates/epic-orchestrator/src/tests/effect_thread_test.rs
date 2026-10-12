use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use super::EffectThread;
use crate::executor::{Call, Executor};
use crate::journal::Record;
use crate::model::{Action, Report, Step};
use crate::server::{Engine, Fault};
use crate::socket_loop::Msg;

fn call(key: &str) -> Call {
  Call {
    key: key.into(),
    action: Action::Checkpoint,
    token: format!("t-{key}"),
  }
}

fn effect(rx: &Receiver<Msg>) -> (Call, String) {
  match rx.recv_timeout(Duration::from_secs(10)) {
    Ok(Msg::Effect { call, outcome }) => (call, outcome),
    _ => panic!("no effect outcome"),
  }
}

fn script(dir: &Path, name: &str, body: &str) {
  std::fs::create_dir_all(dir).expect("scripts");
  let path = dir.join(name);
  std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("script");
  std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
}

/// An engine on the source fixture whose merge script waits for `release`,
/// with issue `a` ready on PR 412 and its merge running on the effect thread.
fn blocked_merge(
  root: &Path,
  rx_tx: mpsc::Sender<Msg>,
) -> (crate::source_fix::Fix, Engine, std::path::PathBuf) {
  let fix = crate::source_fix::fixture(true).expect("fixture");
  let (scripts, release) = (root.join("scripts"), root.join("release"));
  script(
    &scripts,
    "merge",
    &format!(
      "while [ ! -f '{}' ]; do sleep 0.05; done; echo '{{\"outcome\":\"applied\"}}'",
      release.display()
    ),
  );
  script(
    &scripts,
    "prompt",
    &format!("touch '{}'", root.join("prompted").display()),
  );
  let mut engine =
    Engine::open(fix.engine_paths.clone(), Some(scripts), Fault::None).expect("open");
  engine.executor = Executor::Thread(EffectThread::spawn(rx_tx));
  crate::support::report_ready(&mut engine);
  (fix, engine, release)
}

#[test]
fn calls_run_one_at_a_time_in_order() {
  let (tx, rx) = mpsc::channel();
  let thread = EffectThread::spawn(tx);
  for key in ["a", "b", "c"] {
    let label = key.to_owned();
    thread
      .send(call(key), Box::new(move || label))
      .expect("send");
  }
  let order: Vec<String> = (0..3).map(|_| effect(&rx).1).collect();
  assert_eq!(order, ["a", "b", "c"]);
}

#[test]
fn a_panicking_call_reports_a_failure_and_the_thread_keeps_going() {
  let (tx, rx) = mpsc::channel();
  let thread = EffectThread::spawn(tx);
  thread
    .send(call("a"), Box::new(|| panic!("boom")))
    .expect("send");
  thread
    .send(call("b"), Box::new(|| "applied".to_owned()))
    .expect("send");
  assert_eq!(effect(&rx).1, "failed: effect panicked");
  assert_eq!(effect(&rx), (call("b"), "applied".to_owned()));
}

#[test]
fn join_waits_for_the_running_call_and_later_sends_fail() {
  let (tx, rx) = mpsc::channel();
  let mut thread = EffectThread::spawn(tx);
  thread
    .send(
      call("a"),
      Box::new(|| {
        std::thread::sleep(Duration::from_millis(100));
        "late".to_owned()
      }),
    )
    .expect("send");
  thread.join();
  assert_eq!(effect(&rx).1, "late");
  assert_eq!(thread.send(call("b"), Box::new(String::new)), Err(()));
}

#[test]
fn a_dropped_listener_stops_the_thread() {
  let (tx, rx) = mpsc::channel();
  let mut thread = EffectThread::spawn(tx);
  drop(rx);
  thread.send(call("a"), Box::new(String::new)).expect("send");
  thread.join();
  assert!(thread.send(call("b"), Box::new(String::new)).is_err());
}

#[test]
fn status_and_reports_stay_live_while_an_effect_blocks_and_its_release_commits() {
  let tmp = tempfile::tempdir().expect("temp");
  let (tx, rx) = mpsc::channel();
  let (_fix, mut engine, release) = blocked_merge(tmp.path(), tx);
  assert_eq!(
    engine.running.as_ref().map(|call| call.action),
    Some(Action::Merge)
  );
  let status = crate::status::document(&engine.world, true, None);
  assert!(status.to_string().contains("\"a\""), "{status}");
  let state_dir = engine.world.issues["a"].state_dir.clone();
  let note = Report {
    key: "a".into(),
    epic: "one".into(),
    state_dir,
    phase: "ready".into(),
    pr: Some(412),
    note: "still green".into(),
  };
  engine.report(&note).expect("report while blocked");
  assert_eq!(engine.world.issues["a"].note, "still green");
  std::fs::write(&release, "").expect("release");
  let (done, outcome) = effect(&rx);
  engine.effect_done(&done, &outcome).expect("commit");
  assert_eq!(engine.world.issues["a"].stage, "cleaning");
  assert_ne!(
    engine.running.as_ref().map(|call| call.action),
    Some(Action::Merge)
  );
}

#[test]
fn a_prompt_parks_behind_a_running_call_while_journal_and_status_steps_run() {
  let tmp = tempfile::tempdir().expect("temp");
  let (tx, rx) = mpsc::channel();
  let (_fix, mut engine, release) = blocked_merge(tmp.path(), tx);
  let journal_before =
    crate::journal::tail(&engine.paths.journal_dir(), std::time::SystemTime::now())
      .expect("journal")
      .len();
  engine.world.outbox.push_back(Step::Call {
    key: "a".into(),
    action: Action::Prompt,
    token: "p-1".into(),
  });
  engine
    .world
    .outbox
    .push_back(Step::Journal(Record::new("event", "stall", "a", "", "")));
  engine
    .world
    .outbox
    .push_back(Step::Status { key: "a".into() });
  engine.pump().expect("pump");
  assert_eq!(engine.parked.len(), 1);
  let journal_after =
    crate::journal::tail(&engine.paths.journal_dir(), std::time::SystemTime::now())
      .expect("journal")
      .len();
  assert_eq!(journal_after, journal_before + 1);
  assert!(!tmp.path().join("prompted").exists());
  std::fs::write(&release, "").expect("release");
  let (done, outcome) = effect(&rx);
  engine.effect_done(&done, &outcome).expect("commit");
  let (prompt, _) = effect(&rx);
  assert_eq!(prompt.action, Action::Prompt);
  engine.effect_done(&prompt, "applied").expect("prompted");
  assert!(tmp.path().join("prompted").exists() && engine.parked.is_empty());
}

#[test]
fn a_draining_engine_starts_no_parked_prompt_after_the_running_call() {
  let tmp = tempfile::tempdir().expect("temp");
  let (tx, rx) = mpsc::channel();
  let (_fix, mut engine, release) = blocked_merge(tmp.path(), tx);
  engine.world.outbox.push_back(Step::Call {
    key: "a".into(),
    action: Action::Prompt,
    token: "p-1".into(),
  });
  engine.pump().expect("pump");
  assert_eq!(engine.parked.len(), 1);
  engine.world.draining = true;
  std::fs::write(&release, "").expect("release");
  let (done, outcome) = effect(&rx);
  engine.effect_done(&done, &outcome).expect("commit");
  assert!(engine.running.is_none());
  assert_eq!(engine.parked.len(), 1);
  assert!(!tmp.path().join("prompted").exists());
}
