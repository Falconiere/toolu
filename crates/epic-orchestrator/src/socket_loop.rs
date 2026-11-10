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
use crate::socket_deadline::{github_ready, prompt_ready, stall_ready, wake_after};
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

/// Shared snapshots, wake signals and integrations for the state thread.
pub(crate) struct StateContext<'a> {
  pub(crate) protocol: u64,
  pub(crate) shared: &'a Mutex<World>,
  pub(crate) changed: &'a Condvar,
  pub(crate) resub: &'a AtomicBool,
  /// Ends the herdr thread's blocking read so it resubscribes at once.
  pub(crate) interrupt: &'a crate::herdr_interrupt::Interrupt,
  pub(crate) babysit: &'a dyn BabysitTick,
}

/// Process control requests, effect outcomes and host facts until shutdown.
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
        engine.dirty = true;
        engine.refresh_clock();
        // `stop` and `replace` issue no more calls and exit once the running one returns.
        stopping |= request_line(engine, &request, &reply, &mut waiters, context)?;
      }
      Ok(Msg::Effect { call, outcome }) => {
        effect_done(engine, &call, &outcome, &mut waiters)?;
      }
      Ok(Msg::Herdr(fact)) => {
        engine.dirty = true;
        handle_fact(engine, fact, context)?;
      }
      Err(RecvTimeoutError::Timeout) => {
        engine.dirty = true;
        on_timeout(engine, &mut waiters, &mut next_tick, context.babysit)?;
      }
      Err(RecvTimeoutError::Disconnected) => return Ok(()),
    }
  }
}

fn effect_done(
  engine: &mut Engine,
  call: &Call,
  outcome: &str,
  waiters: &mut Vec<Waiter>,
) -> Result<(), String> {
  engine.dirty = true;
  engine.refresh_clock();
  if engine.effect_done(call, outcome)? == Stop::Fault {
    return Err("fault".to_owned());
  }
  // Wait-reply failures cannot stop the engine after replies were sent.
  let _replies = satisfy(engine, waiters);
  Ok(())
}

fn handle_fact(engine: &mut Engine, fact: Fact, context: &StateContext<'_>) -> Result<(), String> {
  engine.refresh_clock();
  source::apply(engine, &Env::process(), fact)?;
  if engine.resubscribe {
    engine.resubscribe = false;
    // Publish first: the herdr thread copies the world it resubscribes with.
    publish(context.shared, context.changed, engine)?;
    context.resub.store(true, Ordering::Relaxed);
    context.interrupt.trigger();
  }
  Ok(())
}

fn request_line(
  engine: &mut Engine,
  request: &Value,
  reply: &Sender<Value>,
  waiters: &mut Vec<Waiter>,
  context: &StateContext<'_>,
) -> Result<bool, String> {
  if request.get("op").and_then(Value::as_str) != Some("cleanup") {
    return Ok(dispatch(engine, request, reply, waiters, context.protocol));
  }
  // HTTP readers must see the committed snapshot when cleanup's SSE event arrives.
  let (tx, rx) = std::sync::mpsc::channel();
  let stop = dispatch(engine, request, &tx, waiters, context.protocol);
  publish(context.shared, context.changed, engine)?;
  // A cleanup that only moves the clock still wakes the journal stream.
  context.changed.notify_all();
  let value = rx.try_recv().map_err(|err| err.to_string())?;
  let _sent = reply.send(value);
  Ok(stop)
}

fn publish(shared: &Mutex<World>, changed: &Condvar, engine: &mut Engine) -> Result<(), String> {
  let mut shared = shared
    .lock()
    .map_err(|_err| "status_unavailable".to_owned())?;
  if !engine.dirty {
    return Ok(());
  }
  // shortcut: dirty turns walk World, upgrade when a profile shows that walk
  let clock_only = same_structure(&shared, &mut engine.world);
  engine.dirty = false;
  if clock_only {
    if shared.now_ms != engine.world.now_ms {
      engine.world.generation = engine.world.generation.wrapping_add(1);
      shared.now_ms = engine.world.now_ms;
      shared.generation = engine.world.generation;
    }
    return Ok(());
  }
  engine.world.generation = engine.world.generation.wrapping_add(1);
  *shared = engine.world.clone();
  changed.notify_all();
  Ok(())
}

/// Content equality, ignoring the clock and the cache generation.
fn same_structure(published: &World, current: &mut World) -> bool {
  let now_ms = current.now_ms;
  let generation = current.generation;
  current.now_ms = published.now_ms;
  current.generation = published.generation;
  let same = *published == *current;
  current.now_ms = now_ms;
  current.generation = generation;
  same
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
  // Whatever woke the loop, a due prompt goes out now, not a tick later.
  if prompt_ready(&engine.world) {
    flush_prompts(engine)?;
  }
  let _replies = satisfy(engine, waiters);
  Ok(())
}

/// Move due queued worker prompts to the outbox and deliver them.
fn flush_prompts(engine: &mut Engine) -> Result<(), String> {
  crate::schedule::flush_prompts(&mut engine.world);
  engine.flush()
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

#[cfg(test)]
#[path = "tests/socket_publish_test.rs"]
mod publish_tests;
