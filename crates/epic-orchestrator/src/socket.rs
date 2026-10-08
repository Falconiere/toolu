//! The resident control socket: one accept thread, one state thread, a 30s tick.

use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{
  Arc,
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
use crate::paths::Paths;
use crate::server::{Engine, Fault, Stop};

pub(crate) struct Waiter {
  pub(crate) deadline: Instant,
  pub(crate) reply: Sender<Value>,
}

enum Msg {
  Line {
    request: Value,
    reply: Sender<Value>,
  },
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
  let accept = thread::spawn(move || accept_loop(listener, tx, flag, protocol));
  let result = {
    let _stop_accept = StopAccept { stop: &stop };
    state_loop(&mut engine, &rx, protocol)
  };
  stop.store(true, Ordering::Relaxed);
  let _wake = UnixStream::connect(&socket);
  if let Err(err) = accept.join() {
    let _panic = err;
  }
  result
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

fn state_loop(engine: &mut Engine, rx: &Receiver<Msg>, protocol: u64) -> Result<(), String> {
  let mut waiters = Vec::new();
  let mut next_tick = Instant::now() + TICK;
  loop {
    let wake = wake_after(&waiters, next_tick);
    match rx.recv_timeout(wake) {
      Ok(Msg::Line { request, reply }) => {
        engine.refresh_clock();
        if dispatch(engine, &request, &reply, &mut waiters, protocol) {
          return Ok(());
        }
      }
      Err(RecvTimeoutError::Timeout) => on_timeout(engine, &mut waiters, &mut next_tick)?,
      Err(RecvTimeoutError::Disconnected) => return Ok(()),
    }
  }
}

fn on_timeout(
  engine: &mut Engine,
  waiters: &mut Vec<Waiter>,
  next_tick: &mut Instant,
) -> Result<(), String> {
  expire(engine, waiters);
  if Instant::now() >= *next_tick {
    tick(engine, next_tick)?;
  }
  satisfy(engine, waiters);
  Ok(())
}

fn tick(engine: &mut Engine, next_tick: &mut Instant) -> Result<(), String> {
  if engine.tick()? == Stop::Fault {
    return Err("fault".to_owned());
  }
  engine.probe_herdr()?;
  *next_tick = Instant::now() + TICK;
  Ok(())
}

fn wake_after(waiters: &[Waiter], next_tick: Instant) -> Duration {
  let until_tick = next_tick.saturating_duration_since(Instant::now());
  let Some(nearest) = waiters.iter().map(|waiter| waiter.deadline).min() else {
    return until_tick;
  };
  until_tick.min(nearest.saturating_duration_since(Instant::now()))
}

#[cfg(test)]
#[path = "tests/socket_test.rs"]
mod tests;
