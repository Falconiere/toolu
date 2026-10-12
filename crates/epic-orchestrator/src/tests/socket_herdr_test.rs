use crate::model::World;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, mpsc};
use toolu_runtime::env::Env;

use super::pause;

#[test]
fn stopped_subscription_does_not_pause() {
  let stop = Arc::new(AtomicBool::new(true));
  let started = std::time::Instant::now();
  pause(&stop, 30);
  assert!(started.elapsed() < std::time::Duration::from_secs(1));
}

#[test]
fn poisoned_shared_world_ends_subscription() {
  let (tx, _rx) = mpsc::channel();
  let shared = Arc::new(Mutex::new(World::new(0)));
  let poisoned = Arc::clone(&shared);
  let _poisoned = std::thread::spawn(move || {
    let _guard = poisoned.lock().expect("world lock");
    panic!("poison world");
  })
  .join();
  let stop = Arc::new(AtomicBool::new(false));
  let wake = Arc::new(AtomicBool::new(false));
  super::run(
    &tx,
    &shared,
    &stop,
    &Env::from_pairs([("HOME", "/tmp")]),
    &wake,
  );
}
