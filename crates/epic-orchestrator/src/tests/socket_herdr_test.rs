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

/// `run` against the scripted herdr peer, logging the methods it receives.
fn live() -> (
  crate::source_fix::Fix,
  std::path::PathBuf,
  std::thread::JoinHandle<()>,
) {
  let mut fix = crate::source_fix::fixture(true).expect("fixture");
  let log = fix.tmp.path().join("methods.log");
  fix.script.log = Some(log.clone());
  let listener = fix.listener.try_clone().expect("listener");
  let peer = crate::source_fix::serve_peer(listener, fix.script.clone());
  (fix, log, peer)
}

fn subscriptions(log: &std::path::Path) -> usize {
  std::fs::read_to_string(log)
    .unwrap_or_default()
    .lines()
    .filter(|line| *line == "events.subscribe")
    .count()
}

fn until(limit: Duration, done: impl Fn() -> bool) -> bool {
  let started = Instant::now();
  while started.elapsed() < limit {
    if done() {
      return true;
    }
    std::thread::sleep(Duration::from_millis(10));
  }
  done()
}

#[test]
fn a_stop_ends_a_live_read_at_once_and_a_resubscribe_reconnects_once() {
  let (fix, log, _peer) = live();
  let signals = signals(false);
  let (tx, _rx) = mpsc::channel();
  let shared = Arc::new(Mutex::new(World::new(0)));
  let env = fix.env;
  let running = signals.clone();
  let thread = std::thread::spawn(move || super::run(&tx, &shared, &env, &running));
  assert!(until(Duration::from_secs(2), || subscriptions(&log) == 1));
  // The peer holds the stream for 2 s; a resubscribe must not wait for it.
  signals
    .wake
    .store(true, std::sync::atomic::Ordering::Relaxed);
  signals.interrupt.trigger();
  assert!(until(Duration::from_millis(900), || subscriptions(&log) == 2));
  std::thread::sleep(Duration::from_millis(300));
  assert_eq!(
    subscriptions(&log),
    2,
    "one reconnect, with no wasted session"
  );
  let stopping = Instant::now();
  signals
    .stop
    .store(true, std::sync::atomic::Ordering::Relaxed);
  signals.interrupt.trigger();
  thread.join().expect("joined");
  assert!(
    stopping.elapsed() < Duration::from_secs(1),
    "{:?}",
    stopping.elapsed()
  );
}
