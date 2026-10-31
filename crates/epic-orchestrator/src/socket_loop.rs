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
  /// A remote result worker finished.
  HcResult {
    key: String,
    result: Result<Option<Value>, String>,
  },
  /// The effect thread finished a call.
  Effect { call: Call, outcome: String },
}

/// Shared snapshots, wake signals and integrations for the state thread.
pub(crate) struct StateContext<'a> {
  pub(crate) protocol: u64,
  pub(crate) sender: &'a Sender<Msg>,
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
  let mut hc_running = std::collections::BTreeSet::new();
  loop {
    publish(context.shared, context.changed, engine)?;
    if stopping && engine.running.is_none() && hc_running.is_empty() {
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
        engine.dirty = true;
        engine.refresh_clock();
        if engine.effect_done(&call, &outcome)? == Stop::Fault {
          return Err("fault".to_owned());
        }
        // Best effort: the replies were already sent, so a wait-reply the
        // journal cannot record must not stop the engine (as in
        // `notify::record`).
        let _replies = satisfy(engine, &mut waiters);
      }
      Ok(Msg::HcResult { key, result }) => {
        engine.dirty = true;
        hc_running.remove(&key);
        crate::hc_results::complete(engine, &key, result)?;
      }
      Ok(Msg::Herdr(fact)) => {
        engine.dirty = true;
        handle_fact(engine, fact, context, (&mut hc_running, stopping))?;
      }
      Err(RecvTimeoutError::Timeout) => {
        engine.dirty = true;
        on_timeout(engine, &mut waiters, &mut next_tick, context.babysit)?;
      }
      Err(RecvTimeoutError::Disconnected) => return Ok(()),
    }
  }
}

fn handle_fact(
  engine: &mut Engine,
  fact: Fact,
  context: &StateContext<'_>,
  status: (&mut std::collections::BTreeSet<String>, bool),
) -> Result<(), String> {
  let (running, stopping) = status;
  engine.refresh_clock();
  let target = hc_event_target(engine, &fact);
  source::apply(engine, &Env::process(), fact)?;
  if !stopping
    && let Some(key) = target
    && running.insert(key.clone())
  {
    spawn_hc_result(engine, &key, context.sender);
  }
  if engine.resubscribe {
    engine.resubscribe = false;
    // Publish first: the herdr thread copies the world it resubscribes with.
    publish(context.shared, context.changed, engine)?;
    context.resub.store(true, Ordering::Relaxed);
    context.interrupt.trigger();
  }
  Ok(())
}

fn hc_event_target(engine: &Engine, fact: &Fact) -> Option<String> {
  let Fact::Event(line) = fact else {
    return None;
  };
  if line.get("event").and_then(Value::as_str) != Some("pane.agent_status_changed") {
    return None;
  }
  let pane = line.pointer("/data/pane_id").and_then(Value::as_str)?;
  let status = line.pointer("/data/agent_status").and_then(Value::as_str)?;
  if !pane.starts_with("hc:") || !matches!(status, "idle" | "blocked" | "done") {
    return None;
  }
  let issue = engine
    .world
    .issues
    .values()
    .find(|issue| issue.agent == pane && issue.stage == "running")?;
  let path = crate::snapshot::issue_path(&issue.state_dir, &issue.key);
  let record = crate::launch::record::load_launch_record(&path).ok()?;
  if crate::launch::record::text(&record, "machine").is_none()
    || crate::launch::record::text(&record, "remote_report").is_none()
    || (record.get("controller_review") == Some(&Value::Bool(true))
      && crate::launch::record::text(&record, "review_prompt_state") == Some("acknowledged"))
  {
    return None;
  }
  Some(issue.key.clone())
}

fn spawn_hc_result(engine: &Engine, key: &str, sender: &Sender<Msg>) {
  let Some(issue) = engine.world.issues.get(key) else {
    return;
  };
  let state = std::path::PathBuf::from(&issue.state_dir);
  let key = key.to_owned();
  let sender = sender.clone();
  let env = engine.env.clone();
  std::thread::spawn(move || {
    let result = std::panic::catch_unwind(|| crate::hc_results::execute(&state, &key, &env))
      .unwrap_or_else(|_| Err("remote result worker panicked".into()));
    let _sent = sender.send(Msg::HcResult { key, result });
  });
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
