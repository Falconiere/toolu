//! The engine's state loop: control requests, herdr facts and effect outcomes
//! arrive as messages; a 30 s tick drives deadlines. Only this thread touches
//! the engine's state.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::sync::{Condvar, Mutex};
use std::time::Instant;

use serde_json::Value;
use toolu_engine::babysit::BabysitTick;
use toolu_runtime::env::Env;

use crate::TICK;
use crate::dispatch::dispatch;
use crate::executor::Call;
use crate::model::World;
use crate::server::{Engine, Stop};
use crate::socket_deadline::{github_ready, stall_ready, wake_after};
use crate::source::{self, Fact};
use crate::wait_ops::{expire, satisfy};

/// A pending `toolu epic wait` reply.
pub(crate) struct Waiter {
  pub(crate) deadline: Instant,
  pub(crate) reply: Sender<Value>,
  /// The registered epic it waits for; `None` waits for every epic.
  pub(crate) epic: Option<String>,
}

/// Request or host event delivered to the state loop.
pub(crate) enum Msg {
  /// One control-socket request and its reply channel.
  Line {
    request: Value,
    reply: Sender<Value>,
  },
  /// A herdr fact from the subscription.
  Herdr(Fact),
  /// The effect thread finished a call.
  Effect { call: Call, outcome: String },
}

pub(crate) struct StateContext<'a> {
  pub(crate) protocol: u64,
  pub(crate) shared: &'a Mutex<World>,
  pub(crate) changed: &'a Condvar,
  pub(crate) resub: &'a AtomicBool,
  /// Ends the herdr thread's blocking read so it resubscribes at once.
  pub(crate) interrupt: &'a crate::herdr_interrupt::Interrupt,
  pub(crate) babysit: &'a dyn BabysitTick,
}

pub(crate) fn state_loop(
  engine: &mut Engine,
  rx: &Receiver<Msg>,
  context: &StateContext<'_>,
) -> Result<(), String> {
  let mut waiters = Vec::new();
  let mut next_tick = Instant::now() + TICK;
  let mut stopping = false;
  loop {
    publish(context.shared, context.changed, engine)?;
    if stopping && engine.running.is_none() {
      return Ok(());
    }
    let wake = wake_after(&engine.world, &waiters, next_tick);
    match rx.recv_timeout(wake) {
      Ok(Msg::Line { request, reply }) => {
        engine.refresh_clock();
        // `stop` and `replace` issue no more calls and exit once the running one returns.
        stopping |= dispatch(engine, &request, &reply, &mut waiters, context.protocol);
      }
      Ok(Msg::Effect { call, outcome }) => {
        engine.refresh_clock();
        if engine.effect_done(&call, &outcome)? == Stop::Fault {
          return Err("fault".to_owned());
        }
        // Best effort: the replies were already sent, so a wait-reply the
        // journal cannot record must not stop the engine (as in
        // `notify::record`).
        let _replies = satisfy(engine, &mut waiters);
      }
      Ok(Msg::Herdr(fact)) => {
        engine.refresh_clock();
        source::apply(engine, &Env::process(), fact)?;
        if engine.resubscribe {
          engine.resubscribe = false;
          // Publish first: the herdr thread copies the world it resubscribes with.
          publish(context.shared, context.changed, engine)?;
          context.resub.store(true, Ordering::Relaxed);
          context.interrupt.trigger();
        }
      }
      Err(RecvTimeoutError::Timeout) => {
        on_timeout(engine, &mut waiters, &mut next_tick, context.babysit)?;
      }
      Err(RecvTimeoutError::Disconnected) => return Ok(()),
    }
  }
}

fn publish(shared: &Mutex<World>, changed: &Condvar, engine: &Engine) -> Result<(), String> {
  *shared
    .lock()
    .map_err(|_err| "status_unavailable".to_owned())? = engine.world.clone();
  changed.notify_all();
  Ok(())
}

fn on_timeout(
  engine: &mut Engine,
  waiters: &mut Vec<Waiter>,
  next_tick: &mut Instant,
  babysit: &dyn BabysitTick,
) -> Result<(), String> {
  engine.refresh_clock();
  // Best effort, as above: expired waiters were already replied.
  let _expired = expire(engine, waiters);
  if Instant::now() >= *next_tick {
    tick(engine, next_tick, babysit)?;
  } else if stall_ready(&engine.world) || github_ready(&engine.world) {
    let _tick = engine.tick(babysit)?;
  }
  let _replies = satisfy(engine, waiters);
  Ok(())
}

fn tick(
  engine: &mut Engine,
  next_tick: &mut Instant,
  babysit: &dyn BabysitTick,
) -> Result<(), String> {
  if engine.tick(babysit)? == Stop::Fault {
    return Err("fault".to_owned());
  }
  *next_tick = Instant::now() + TICK;
  Ok(())
}

#[cfg(test)]
#[path = "tests/socket_loop_test.rs"]
mod tests;
