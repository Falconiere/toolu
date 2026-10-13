//! Herdr subscription thread for the resident engine.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc::Sender};
use std::thread;
use std::time::{Duration, Instant};

use toolu_runtime::env::Env;

use crate::herdr_interrupt::{IDLE, Interrupt};
use crate::model::World;
use crate::socket_loop::Msg;
use crate::source::{self, Pace};

/// What ends a subscription read or the pause between sessions.
#[derive(Debug, Clone, Default)]
pub(crate) struct Signals {
  /// The engine is stopping.
  pub stop: Arc<AtomicBool>,
  /// A new pane needs a fresh subscription.
  pub wake: Arc<AtomicBool>,
  /// Shuts the live stream down and ends a pause.
  pub interrupt: Arc<Interrupt>,
}

/// Spawn the Herdr subscription when prompt events are available.
pub(crate) fn spawn(
  tx: Sender<Msg>,
  shared: Arc<Mutex<World>>,
  signals: Signals,
) -> Option<thread::JoinHandle<()>> {
  if !source::prompt_enabled() {
    return None;
  }
  let env = Env::process();
  Some(thread::spawn(move || {
    run(&tx, &shared, &env, &signals);
  }))
}

/// Subscribe until stopped. A quiet socket waits [`IDLE`]; a missing socket
/// waits [`IDLE`] between attempts; a session that failed at once waits 1 s.
/// Every wait ends early on a trigger, so no short timer runs while idle.
fn run(tx: &Sender<Msg>, shared: &Arc<Mutex<World>>, env: &Env, signals: &Signals) {
  while !signals.stop.load(Ordering::Relaxed) {
    let world = match shared.lock() {
      Ok(world) => world.clone(),
      Err(_err) => break,
    };
    signals.interrupt.reset();
    // This session subscribes with the current world, so an older request
    // to resubscribe is already served.
    signals.wake.store(false, Ordering::Relaxed);
    let pace = Pace {
      idle: Some(IDLE),
      return_on_idle: false,
      stop: Some(Arc::clone(&signals.stop)),
      resubscribe: Some(Arc::clone(&signals.wake)),
      interrupt: Some(Arc::clone(&signals.interrupt)),
    };
    let missing = !crate::herdr::socket_path(env).exists();
    let started = Instant::now();
    let _ran = source::session(env, &world, &pace, |fact| {
      tx.send(Msg::Herdr(fact)).map_err(|err| err.to_string())
    });
    if signals.stop.load(Ordering::Relaxed) {
      break;
    }
    // A session that failed at once (an unreadable reply, a refused
    // connection) is not retried in a tight loop.
    if missing {
      signals.interrupt.pause(IDLE);
    } else if started.elapsed() < Duration::from_secs(1) {
      signals.interrupt.pause(Duration::from_secs(1));
    }
  }
}

#[cfg(test)]
#[path = "tests/socket_herdr_test.rs"]
mod tests;
