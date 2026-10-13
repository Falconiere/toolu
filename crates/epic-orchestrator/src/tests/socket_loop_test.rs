use std::sync::mpsc;
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

use super::{Msg, Waiter, on_timeout, publish};
use crate::effect_thread::EffectThread;
use crate::executor::{Call, Executor};
use crate::model::{Action, World};
use crate::paths::Paths;
use crate::server::{Engine, Fault};

#[test]
fn poisoned_snapshot_publish_returns_error() {
  let root = tempfile::tempdir().expect("root");
  let engine = Engine::open(Paths::at(root.path()), None, Fault::None).expect("engine");
  let shared = Mutex::new(World::new(0));
  let _poisoned = std::panic::catch_unwind(|| {
    let _guard = shared.lock().expect("shared lock");
    panic!("poison snapshot");
  });
  let changed = Condvar::new();
  assert_eq!(
    publish(&shared, &changed, &engine),
    Err("status_unavailable".to_owned())
  );
}

#[test]
fn an_effect_outcome_reaches_the_loop_as_a_message() {
  let (tx, rx) = mpsc::channel();
  let thread = EffectThread::spawn(tx);
  let executor = Executor::Thread(thread);
  let call = Call {
    key: "k-1".into(),
    action: Action::Checkpoint,
    token: "t".into(),
  };
  executor.submit(call.clone(), Box::new(|| "applied".to_owned()));
  match rx.recv_timeout(Duration::from_secs(5)) {
    Ok(Msg::Effect {
      call: done,
      outcome,
    }) => assert_eq!((done, outcome.as_str()), (call, "applied")),
    _ => panic!("no effect message"),
  }
}

#[test]
fn a_wait_reply_the_journal_cannot_record_still_replies_and_continues() {
  let root = tempfile::tempdir().expect("root");
  let mut engine = Engine::open(Paths::at(root.path()), None, Fault::None).expect("engine");
  // A file where the journal directory would go, so every append fails.
  std::fs::write(Paths::at(root.path()).journal_dir(), "not a directory").expect("block");
  let (reply, rx) = mpsc::channel();
  let mut waiters = vec![Waiter {
    deadline: Instant::now(),
    reply,
  }];
  let mut next_tick = Instant::now() + Duration::from_secs(30);
  assert_eq!(
    on_timeout(&mut engine, &mut waiters, &mut next_tick, &()),
    Ok(())
  );
  assert_eq!(rx.recv().expect("reply")["state"], "waiting");
  assert!(waiters.is_empty());
}
