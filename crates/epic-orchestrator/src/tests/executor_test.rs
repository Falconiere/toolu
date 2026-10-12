use std::sync::mpsc;
use std::time::Duration;

use super::{Call, Executor, STOPPED, Submitted};
use crate::effect_thread::EffectThread;
use crate::model::Action;
use crate::socket_loop::Msg;

fn call() -> Call {
  Call {
    key: "k-1".into(),
    action: Action::Launch,
    token: "t".into(),
  }
}

#[test]
fn inline_runs_the_call_at_once() {
  assert_eq!(
    Executor::Inline.submit(call(), Box::new(|| "applied".to_owned())),
    Submitted::Done("applied".to_owned())
  );
}

#[test]
fn the_thread_takes_the_call_and_answers_later() {
  let (tx, rx) = mpsc::channel();
  let executor = Executor::Thread(EffectThread::spawn(tx));
  assert_eq!(
    executor.submit(call(), Box::new(|| "wait".to_owned())),
    Submitted::Sent
  );
  assert!(
    matches!(rx.recv_timeout(Duration::from_secs(5)), Ok(Msg::Effect { outcome, .. }) if outcome == "wait")
  );
}

#[test]
fn a_stopped_thread_fails_every_later_call() {
  let (tx, _rx) = mpsc::channel();
  let mut thread = EffectThread::spawn(tx);
  thread.join();
  let executor = Executor::Thread(thread);
  assert_eq!(
    executor.submit(call(), Box::new(|| "applied".to_owned())),
    Submitted::Done(STOPPED.to_owned())
  );
  assert_eq!(STOPPED, "failed: effect thread stopped");
}

#[test]
fn an_inline_engine_commits_a_script_outcome_without_a_thread() {
  let fix = crate::source_fix::fixture(true).expect("fixture");
  let mut engine =
    crate::server::Engine::open(fix.engine_paths, None, crate::server::Fault::None).expect("open");
  crate::support::report_ready(&mut engine);
  assert!(engine.running.is_none());
  assert_eq!(engine.world.issues["a"].deferred.as_deref(), Some("merge"));
}
