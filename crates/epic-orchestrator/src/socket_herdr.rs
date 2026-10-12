//! Herdr subscription thread for the resident engine.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc::Sender};
use std::thread;
use std::time::{Duration, Instant};

use toolu_runtime::env::Env;

use crate::model::World;
use crate::socket_loop::Msg;
use crate::source::{self, Pace};

/// Spawn the Herdr subscription when prompt events are available.
pub(crate) fn spawn(
  tx: Sender<Msg>,
  shared: Arc<Mutex<World>>,
  stop: Arc<AtomicBool>,
  wake: Arc<AtomicBool>,
) -> Option<thread::JoinHandle<()>> {
  if !source::prompt_enabled() {
    return None;
  }
  let env = Env::process();
  Some(thread::spawn(move || {
    run(&tx, &shared, &stop, &env, &wake);
  }))
}

fn run(
  tx: &Sender<Msg>,
  shared: &Arc<Mutex<World>>,
  stop: &Arc<AtomicBool>,
  env: &Env,
  wake: &Arc<AtomicBool>,
) {
  while !stop.load(Ordering::Relaxed) {
    let world = match shared.lock() {
      Ok(world) => world.clone(),
      Err(_err) => break,
    };
    let pace = Pace {
      idle: Some(Duration::from_secs(1)),
      return_on_idle: false,
      stop: Some(Arc::clone(stop)),
      resubscribe: Some(Arc::clone(wake)),
    };
    let missing = !crate::herdr::socket_path(env).exists();
    let started = Instant::now();
    let _ran = source::session(env, &world, &pace, |fact| {
      tx.send(Msg::Herdr(fact)).map_err(|err| err.to_string())
    });
    // A session that failed at once (an unreadable reply, a refused
    // connection) is not retried in a tight loop.
    if missing {
      pause(stop, 30);
    } else if started.elapsed() < Duration::from_secs(1) {
      pause(stop, 1);
    }
  }
}

fn pause(stop: &Arc<AtomicBool>, seconds: u32) {
  for _ in 0..seconds {
    if stop.load(Ordering::Relaxed) {
      return;
    }
    thread::sleep(Duration::from_secs(1));
  }
}

#[cfg(test)]
#[path = "tests/socket_herdr_test.rs"]
mod tests;
