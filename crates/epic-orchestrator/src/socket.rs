//! The resident control socket: one accept thread, one state thread, a 30s tick.

use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{
  Arc, Mutex,
  mpsc::{self, Receiver, RecvTimeoutError, Sender},
};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use toolu_runtime::env::Env;

use crate::TICK;
use crate::client::{read_json, write_json};
use crate::dispatch::{dispatch, expire, ingest, satisfy};
use crate::lock::Held;
use crate::model::World;
use crate::paths::Paths;
use crate::schedule::next_stall_ms;
use crate::server::{Engine, Fault, Stop};
use crate::source::{self, Fact, Pace};

/// A pending `toolu epic wait` reply.
pub(crate) struct Waiter {
  pub(crate) deadline: Instant,
  pub(crate) reply: Sender<Value>,
}

enum Msg {
  Line {
    request: Value,
    reply: Sender<Value>,
  },
  Herdr(Fact),
}

/// Sets the accept-loop stop flag if `state_loop` unwinds before `serve` joins it.
struct StopAccept<'a> {
  stop: &'a AtomicBool,
}

impl Drop for StopAccept<'_> {
  fn drop(&mut self) {
    self.stop.store(true, Ordering::Relaxed);
  }
}

/// Listen until `stop` or `replace`. The lock is released when this returns.
///
/// # Errors
/// The lock is busy, the socket cannot be bound, or a fault fires.
pub(crate) fn serve(
  paths: Paths,
  scripts: Option<std::path::PathBuf>,
  fault: Fault,
  protocol: u64,
) -> Result<(), String> {
  let socket = paths.socket();
  let _held = Held::acquire(&paths.lock(), "engine-busy")?;
  let mut engine = Engine::open(paths, scripts, fault)?;
  apply_env(&mut engine);
  ingest(&mut engine)?;
  if engine.pump()? == Stop::Fault {
    return Err("fault".to_owned());
  }
  let listener = bind(&engine.paths)?;
  let (tx, rx) = mpsc::channel();
  let stop = Arc::new(AtomicBool::new(false));
  let flag = Arc::clone(&stop);
  let shared = Arc::new(Mutex::new(engine.world.clone()));
  let wake = Arc::new(AtomicBool::new(false));
  let herdr = spawn_herdr(
    tx.clone(),
    Arc::clone(&shared),
    Arc::clone(&stop),
    Arc::clone(&wake),
  );
  let accept = thread::spawn(move || accept_loop(listener, tx, flag, protocol));
  let result = {
    let _stop_accept = StopAccept { stop: &stop };
    state_loop(&mut engine, &rx, protocol, &shared, &wake)
  };
  stop.store(true, Ordering::Relaxed);
  let _wake = UnixStream::connect(&socket);
  if let Err(err) = accept.join() {
    let _panic = err;
  }
  if let Some(herdr) = herdr
    && let Err(err) = herdr.join()
  {
    let _panic = err;
  }
  result
}

fn spawn_herdr(
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
    herdr_loop(&tx, &shared, &stop, &env, &wake);
  }))
}

fn herdr_loop(
  tx: &Sender<Msg>,
  shared: &Arc<Mutex<World>>,
  stop: &Arc<AtomicBool>,
  env: &Env,
  wake: &Arc<AtomicBool>,
) {
  while !stop.load(Ordering::Relaxed) {
    let world = shared
      .lock()
      .unwrap_or_else(std::sync::PoisonError::into_inner)
      .clone();
    let pace = Pace {
      idle: Some(Duration::from_secs(1)),
      return_on_idle: false,
      stop: Some(Arc::clone(stop)),
      resubscribe: Some(Arc::clone(wake)),
    };
    let missing = !crate::herdr::socket_path(env).exists();
    let _ran = source::session(env, &world, &pace, |fact| {
      tx.send(Msg::Herdr(fact)).map_err(|err| err.to_string())
    });
    if missing {
      pause(stop);
    }
  }
}

fn pause(stop: &Arc<AtomicBool>) {
  for _ in 0..30 {
    if stop.load(Ordering::Relaxed) {
      return;
    }
    thread::sleep(Duration::from_secs(1));
  }
}

fn apply_env(engine: &mut Engine) {
  let env = Env::process();
  engine.herdr_session = env.get("TOOLU_EPIC_HERDR_SESSION").map(str::to_owned);
  engine.git_trace = env
    .get("TOOLU_EPIC_GIT_TRACE")
    .map(std::path::PathBuf::from);
}

fn bind(paths: &Paths) -> Result<UnixListener, String> {
  let path = paths.socket();
  if path.exists() {
    std::fs::remove_file(&path).map_err(|err| err.to_string())?;
  }
  let listener = UnixListener::bind(&path).map_err(|err| err.to_string())?;
  let mut perms = std::fs::metadata(&path)
    .map_err(|err| err.to_string())?
    .permissions();
  perms.set_mode(0o600);
  std::fs::set_permissions(&path, perms).map_err(|err| err.to_string())?;
  Ok(listener)
}

fn accept_loop(listener: UnixListener, tx: Sender<Msg>, stop: Arc<AtomicBool>, protocol: u64) {
  for stream in listener.incoming() {
    if stop.load(Ordering::Relaxed) {
      break;
    }
    let Ok(stream) = stream else { break };
    let tx = tx.clone();
    thread::spawn(move || session(stream, &tx, protocol));
  }
  drop(listener);
  drop(tx);
  drop(stop);
}

fn session(mut stream: UnixStream, tx: &Sender<Msg>, protocol: u64) {
  if write_json(&mut stream, &json!({"op": "hello", "protocol": protocol})).is_err() {
    return;
  }
  let Ok(request) = read_json(&mut stream) else {
    return;
  };
  hold_before_ack();
  let (reply, rx) = mpsc::channel();
  if tx.send(Msg::Line { request, reply }).is_err() {
    return;
  }
  if let Ok(value) = rx.recv() {
    let _wrote = write_json(&mut stream, &value);
  }
}

fn hold_before_ack() {
  if Env::process().get("TOOLU_EPIC_HOLD") != Some("before-ack") {
    return;
  }
  loop {
    thread::sleep(Duration::from_millis(50));
  }
}

fn state_loop(
  engine: &mut Engine,
  rx: &Receiver<Msg>,
  protocol: u64,
  shared: &Mutex<World>,
  resub: &AtomicBool,
) -> Result<(), String> {
  let mut waiters = Vec::new();
  let mut next_tick = Instant::now() + TICK;
  loop {
    publish(shared, engine);
    let wake = wake_after(engine, &waiters, next_tick);
    match rx.recv_timeout(wake) {
      Ok(Msg::Line { request, reply }) => {
        engine.refresh_clock();
        if dispatch(engine, &request, &reply, &mut waiters, protocol) {
          return Ok(());
        }
      }
      Ok(Msg::Herdr(fact)) => {
        engine.refresh_clock();
        source::apply(engine, &Env::process(), fact)?;
        if engine.resubscribe {
          engine.resubscribe = false;
          resub.store(true, Ordering::Relaxed);
        }
      }
      Err(RecvTimeoutError::Timeout) => on_timeout(engine, &mut waiters, &mut next_tick)?,
      Err(RecvTimeoutError::Disconnected) => return Ok(()),
    }
  }
}

fn publish(shared: &Mutex<World>, engine: &Engine) {
  *shared
    .lock()
    .unwrap_or_else(std::sync::PoisonError::into_inner) = engine.world.clone();
}

fn on_timeout(
  engine: &mut Engine,
  waiters: &mut Vec<Waiter>,
  next_tick: &mut Instant,
) -> Result<(), String> {
  expire(engine, waiters);
  if Instant::now() >= *next_tick {
    tick(engine, next_tick)?;
  } else if stall_ready(engine) || github_ready(engine) {
    engine.refresh_clock();
    let _tick = engine.tick()?;
  }
  satisfy(engine, waiters);
  Ok(())
}

fn tick(engine: &mut Engine, next_tick: &mut Instant) -> Result<(), String> {
  if engine.tick()? == Stop::Fault {
    return Err("fault".to_owned());
  }
  *next_tick = Instant::now() + TICK;
  Ok(())
}

fn stall_ready(engine: &Engine) -> bool {
  next_stall_ms(&engine.world).is_some_and(|at| at <= engine.world.now_ms)
}

fn github_ready(engine: &Engine) -> bool {
  engine
    .world
    .watches
    .values()
    .any(|watch| watch.due_at_ms(engine.world.now_ms) <= engine.world.now_ms)
}

fn wake_after(engine: &Engine, waiters: &[Waiter], next_tick: Instant) -> Duration {
  let until_tick = next_tick.saturating_duration_since(Instant::now());
  let until_wait = waiters
    .iter()
    .map(|waiter| waiter.deadline)
    .min()
    .map(|deadline| deadline.saturating_duration_since(Instant::now()));
  let until_stall = next_stall_ms(&engine.world)
    .map(|at| Duration::from_millis(at.saturating_sub(engine.world.now_ms)));
  let wake = until_wait.map_or(until_tick, |wait| until_tick.min(wait));
  let wake = until_stall.map_or(wake, |stall| wake.min(stall));
  let until_github = engine
    .world
    .watches
    .values()
    .map(|watch| watch.due_at_ms(engine.world.now_ms))
    .min()
    .map(|at| Duration::from_millis(at.saturating_sub(engine.world.now_ms)));
  until_github.map_or(wake, |github| wake.min(github))
}

#[cfg(test)]
#[path = "tests/socket_test.rs"]
mod tests;
