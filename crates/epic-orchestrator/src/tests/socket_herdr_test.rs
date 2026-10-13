use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use toolu_runtime::env::Env;

use super::Signals;
use crate::herdr_interrupt::{IDLE, Interrupt};
use crate::model::World;

fn signals(stop: bool) -> Signals {
  Signals {
    stop: Arc::new(AtomicBool::new(stop)),
    wake: Arc::new(AtomicBool::new(false)),
    interrupt: Arc::new(Interrupt::default()),
  }
}

#[test]
fn a_stopped_subscription_returns_at_once() {
  let (tx, _rx) = mpsc::channel();
  let started = Instant::now();
  super::run(
    &tx,
    &Arc::new(Mutex::new(World::new(0))),
    &Env::from_pairs([("HOME", "/nonexistent")]),
    &signals(true),
  );
  assert!(started.elapsed() < Duration::from_secs(1));
}

#[test]
fn a_missing_socket_waits_the_idle_interval_until_stopped() {
  assert_eq!(IDLE, Duration::from_secs(30));
  let (tx, rx) = mpsc::channel();
  let signals = signals(false);
  let stop = Arc::clone(&signals.stop);
  let interrupt = Arc::clone(&signals.interrupt);
  let shared = Arc::new(Mutex::new(World::new(0)));
  let started = Instant::now();
  let thread = std::thread::spawn(move || {
    super::run(
      &tx,
      &shared,
      &Env::from_pairs([("HOME", "/nonexistent")]),
      &signals,
    );
  });
  let outage = rx.recv_timeout(Duration::from_secs(5)).expect("outage");
  assert!(matches!(outage, crate::socket_loop::Msg::Herdr(_)));
  std::thread::sleep(Duration::from_millis(300));
  assert!(
    rx.try_recv().is_err(),
    "no second attempt inside the idle interval"
  );
  stop.store(true, std::sync::atomic::Ordering::Relaxed);
  interrupt.trigger();
  thread.join().expect("joined");
  assert!(started.elapsed() < Duration::from_secs(5));
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
  super::run(
    &tx,
    &shared,
    &Env::from_pairs([("HOME", "/tmp")]),
    &signals(false),
  );
}
