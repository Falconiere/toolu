//! The resident control socket: one accept thread, one state thread, a 30s tick.

use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{
  Arc, Condvar, Mutex,
  mpsc::{self, Sender},
};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::json;
use toolu_engine::babysit::BabysitTick;
use toolu_runtime::env::Env;

use crate::client::{read_request, write_json};
use crate::dispatch::ingest;
use crate::effect_thread::EffectThread;
use crate::executor::Executor;
use crate::http::{self, Config as HttpConfig};
use crate::lock::Held;
use crate::model::World;
use crate::notify::AttentionOptions;
use crate::paths::Paths;
use crate::server::{Engine, Fault, Stop};
use crate::socket_herdr;
use crate::socket_loop::{Msg, StateContext, state_loop};

const REQUEST_DEADLINE: Duration = Duration::from_secs(5);

/// Sets the accept-loop stop flag if `state_loop` unwinds before `serve` joins it.
struct StopAccept<'a> {
  stop: &'a AtomicBool,
}

struct Threads {
  accept: thread::JoinHandle<()>,
  herdr: Option<thread::JoinHandle<()>>,
  http: Option<http::Running>,
}

/// HTTP binding, babysit integration and attention settings for one engine run.
pub(crate) struct ServeOptions<'a> {
  pub(crate) http: Option<HttpConfig>,
  pub(crate) tick: &'a dyn BabysitTick,
  pub(crate) attention: AttentionOptions,
}

impl Drop for StopAccept<'_> {
  fn drop(&mut self) {
    self.stop.store(true, Ordering::Relaxed);
  }
}

/// Run the engine and optionally its read-only HTTP listener.
///
/// # Errors
/// The state, control socket or HTTP listener cannot start.
pub(crate) fn serve_with_http(
  paths: Paths,
  scripts: Option<std::path::PathBuf>,
  fault: Fault,
  protocol: u64,
  options: ServeOptions<'_>,
) -> Result<(), String> {
  let socket = paths.socket();
  let _held = Held::acquire(&paths.lock(), "engine-busy")?;
  let (tx, rx) = mpsc::channel();
  let mut engine = open_engine(paths, scripts, fault, options.attention, &tx)?;
  let listener = bind(&engine.paths)?;
  let signals = socket_herdr::Signals::default();
  let shared = Arc::new(Mutex::new(engine.world.clone()));
  let changed = Arc::new(Condvar::new());
  let http = start_http(options.http, &tx, &engine, &signals, (&shared, &changed))?;
  if let (Some(notifier), Some(running)) = (engine.notifier.as_mut(), http.as_ref()) {
    notifier.set_dashboard(running.addr);
  }
  let herdr = socket_herdr::spawn(tx.clone(), Arc::clone(&shared), signals.clone());
  let flag = Arc::clone(&signals.stop);
  let accept = thread::spawn(move || accept_loop(listener, tx, flag, protocol));
  let result = {
    let _stop_accept = StopAccept {
      stop: &signals.stop,
    };
    state_loop(
      &mut engine,
      &rx,
      &StateContext {
        protocol,
        shared: &shared,
        changed: &changed,
        resub: &signals.wake,
        interrupt: &signals.interrupt,
        babysit: options.tick,
      },
    )
  };
  drop(rx);
  let threads = Threads {
    accept,
    herdr,
    http,
  };
  join_threads(&socket, &signals, &changed, threads);
  let _gone = std::fs::remove_file(engine.paths.http());
  stopped(&mut engine, result)
}

/// A clean stop journals the engine's last resource sample.
fn stopped(engine: &mut Engine, result: Result<(), String>) -> Result<(), String> {
  result?;
  engine.refresh_clock();
  crate::engine_usage::sample(engine, "stop")
}

fn join_threads(
  socket: &std::path::Path,
  signals: &socket_herdr::Signals,
  changed: &Condvar,
  threads: Threads,
) {
  let stop = &signals.stop;
  stop.store(true, Ordering::Relaxed);
  changed.notify_all();
  signals.interrupt.trigger();
  let _wake = UnixStream::connect(socket);
  if let Err(err) = threads.accept.join() {
    let _panic = err;
  }
  if let Some(herdr) = threads.herdr
    && let Err(err) = herdr.join()
  {
    let _panic = err;
  }
  if let Some(http) = threads.http {
    let _http = http.stop(stop);
  }
}

fn open_engine(
  paths: Paths,
  scripts: Option<std::path::PathBuf>,
  fault: Fault,
  attention: AttentionOptions,
  effects: &Sender<Msg>,
) -> Result<Engine, String> {
  let mut engine = Engine::open(paths, scripts, fault)?;
  engine.executor = Executor::Thread(EffectThread::spawn(effects.clone()));
  engine.world.remind_ms = attention.remind_ms;
  let journal = engine.paths.clone();
  engine.notifier = attention
    .notify
    .map(|settings| crate::notify::Notifier::spawn(settings, journal));
  apply_env(&mut engine);
  ingest(&mut engine)?;
  if engine.pump()? == Stop::Fault {
    return Err("fault".to_owned());
  }
  crate::engine_usage::sample(&mut engine, "start")?;
  Ok(engine)
}

/// Start the listener with a control path to the state loop, and record its
/// bound address in `http.json` for `toolu epic token url` (it holds no secret).
fn start_http(
  config: Option<HttpConfig>,
  control: &Sender<Msg>,
  engine: &Engine,
  signals: &socket_herdr::Signals,
  (shared, changed): (&Arc<Mutex<World>>, &Arc<Condvar>),
) -> Result<Option<http::Running>, String> {
  // A crashed run leaves its file behind; a `--no-http` engine must not serve that address.
  let _stale = std::fs::remove_file(engine.paths.http());
  let Some(config) = config else {
    return Ok(None);
  };
  let config = HttpConfig {
    control: Some(control.clone()),
    ..config
  };
  let running = http::start(
    config,
    engine.paths.clone(),
    Arc::clone(shared),
    Arc::clone(changed),
    Arc::clone(&signals.stop),
  )?;
  let addr = json!({"addr": running.addr.to_string()});
  if let Err(err) = crate::disk::write_value(&engine.paths.http(), &addr) {
    let _stopped = running.stop(&signals.stop);
    return Err(err);
  }
  Ok(Some(running))
}

fn apply_env(engine: &mut Engine) {
  let env = Env::process();
  let number = |name: &str, fallback: u64| crate::retry::number(&env, name, fallback);
  engine.world.floors = (
    number("EPIC_GH_CORE_FLOOR", 1_000),
    number("EPIC_GH_GRAPHQL_FLOOR", 500),
  );
  engine.world.cooldown_min = number("EPIC_HOST_COOLDOWN_MIN", 60);
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
  thread::scope(|scope| {
    for stream in listener.incoming() {
      if stop.load(Ordering::Relaxed) {
        break;
      }
      let Ok(stream) = stream else { break };
      let deadline = Instant::now() + REQUEST_DEADLINE;
      let tx = tx.clone();
      scope.spawn(move || session(stream, &tx, protocol, deadline));
    }
  });
  drop(listener);
  drop(tx);
  drop(stop);
}

fn session(mut stream: UnixStream, tx: &Sender<Msg>, protocol: u64, deadline: Instant) {
  if write_json(&mut stream, &json!({"op": "hello", "protocol": protocol})).is_err() {
    return;
  }
  let Ok(request) = read_request(&mut stream, deadline) else {
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

#[cfg(test)]
#[path = "tests/socket_test.rs"]
mod tests;

#[cfg(test)]
#[path = "tests/socket_request_test.rs"]
mod request_tests;
