use std::sync::mpsc;
use std::sync::{Condvar, Mutex};
use std::time::Duration;

use super::{Msg, publish};
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
