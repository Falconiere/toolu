//! Bounded TCP listener for epic status and token-gated control actions.

use std::io::BufReader;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use toolu_runtime::config::epic::Peer;
use toolu_runtime::config::secrets::Secrets;
use toolu_runtime::host::roots::Roots;

use crate::http_access::{authorized, host_allowed};
use crate::http_action::{self, Control};
use crate::http_fleet::{FleetCache, REFRESH_EVERY, Refresher};
use crate::http_request::{ReadError, Request, read_request};
use crate::http_response::{OverviewCache, Response, send};
use crate::http_secrets::LiveSecrets;
use crate::model::World;
use crate::paths::Paths;
use crate::socket_loop::Msg;

const CONNECTIONS: usize = 64;
/// The most live event streams served at once.
pub(crate) const STREAMS: usize = 8;

/// Bound address, private status credentials, read-only peers and the
/// control path.
pub(crate) struct Config {
  pub addr: SocketAddr,
  pub secrets: Secrets,
  pub peers: Vec<Peer>,
  /// Where live-agent and worker-screen reads reach herdr.
  pub herdr: crate::http_live::Herdr,
  /// The named herdr session, for a blocked worker's attach command.
  pub attach_session: Option<String>,
  /// Follow later changes to this root's `secrets.json`; `None` keeps `secrets`.
  pub reload: Option<Roots>,
  /// The state loop actions go to; `None` answers them `503 engine_busy`.
  pub control: Option<Sender<Msg>>,
  /// How long an action waits for the state loop.
  pub deadline: Duration,
}

impl Config {
  /// A listener with fixed secrets and no control path.
  pub(crate) fn new(addr: SocketAddr, secrets: Secrets, peers: Vec<Peer>) -> Self {
    Self {
      addr,
      secrets,
      peers,
      reload: None,
      control: None,
      deadline: http_action::DEADLINE,
      herdr: crate::http_live::Herdr::Off,
      attach_session: None,
    }
  }
}

/// A listener thread and its bound address.
pub(crate) struct Running {
  pub addr: SocketAddr,
  pub thread: JoinHandle<()>,
  refresher: Refresher,
}

impl Running {
  /// Raise `stop`, wake the blocking `accept` with one connection, and join.
  pub(crate) fn stop(self, stop: &AtomicBool) -> std::thread::Result<()> {
    stop.store(true, Ordering::Relaxed);
    let _wake = TcpStream::connect(self.addr);
    self.refresher.stop()?;
    self.thread.join()
  }
}

/// What every connection thread shares.
pub(crate) struct Context {
  pub(crate) paths: Paths,
  pub(crate) world: Arc<Mutex<World>>,
  pub(crate) changed: Arc<Condvar>,
  pub(crate) stop: Arc<AtomicBool>,
  pub(crate) secrets: LiveSecrets,
  pub(crate) control: Option<Sender<Msg>>,
  pub(crate) deadline: Duration,
  pub(crate) auth_required: bool,
  pub(crate) started: Instant,
  pub(crate) connections: AtomicUsize,
  pub(crate) streams: AtomicUsize,
  pub(crate) overview: OverviewCache,
  pub(crate) metrics: crate::http_metrics::MetricsCache,
  pub(crate) fleet: Arc<FleetCache>,
  pub(crate) live: crate::http_live::Live,
  pub(crate) screen: crate::http_screen::Screen,
  pub(crate) herdr_calls: crate::http_live::Limiter,
}

/// Bind before returning so a startup bind error stops the engine.
///
/// # Errors
/// The token is absent for a non-loopback bind or the address cannot be bound.
pub(crate) fn start(
  config: Config,
  paths: Paths,
  world: Arc<Mutex<World>>,
  changed: Arc<Condvar>,
  stop: Arc<AtomicBool>,
) -> Result<Running, String> {
  let auth_required = !config.addr.ip().is_loopback();
  if auth_required && config.secrets.status_token().is_none() {
    return Err("non-loopback HTTP bind requires TOOLU_EPIC_TOKEN or a status token".to_owned());
  }
  let listener = TcpListener::bind(config.addr).map_err(|err| format!("HTTP bind: {err}"))?;
  let addr = listener
    .local_addr()
    .map_err(|err| format!("HTTP address: {err}"))?;
  let fleet = Arc::new(FleetCache::new(&config.peers, &config.secrets));
  let refresher = Refresher::spawn(&fleet, &stop, REFRESH_EVERY);
  let context = Arc::new(Context {
    paths,
    world,
    changed,
    stop,
    secrets: LiveSecrets::new(config.secrets, config.reload),
    control: config.control,
    deadline: config.deadline,
    auth_required,
    started: Instant::now(),
    connections: AtomicUsize::new(0),
    streams: AtomicUsize::new(0),
    overview: OverviewCache::new(),
    metrics: crate::http_metrics::MetricsCache::new(),
    fleet,
    live: crate::http_live::Live::new(config.herdr.clone()),
    screen: crate::http_screen::Screen::new(config.herdr, config.attach_session),
    herdr_calls: crate::http_live::Limiter::default(),
  });
  let thread = thread::spawn(move || accept_loop(&listener, &context));
  Ok(Running {
    addr,
    thread,
    refresher,
  })
}

fn accept_loop(listener: &TcpListener, context: &Arc<Context>) {
  while !context.stop.load(Ordering::Relaxed) {
    match listener.accept() {
      // `Running::stop` connects once to end a blocking accept on stop.
      Ok(_) if context.stop.load(Ordering::Relaxed) => break,
      Ok((mut stream, _)) => {
        if stream.set_nodelay(true).is_err() {
          continue;
        }
        let count = context.connections.fetch_add(1, Ordering::AcqRel);
        if count >= CONNECTIONS {
          context.connections.fetch_sub(1, Ordering::AcqRel);
          let _sent = send(&mut stream, &Response::error(503, "connection_limit"));
          continue;
        }
        let context = Arc::clone(context);
        thread::spawn(move || {
          serve_client(&mut stream, &context);
          context.connections.fetch_sub(1, Ordering::AcqRel);
        });
      }
      Err(err) if err.kind() == std::io::ErrorKind::Interrupted => {}
      Err(_err) => break,
    }
  }
}

fn serve_client(stream: &mut TcpStream, context: &Context) {
  if stream
    .set_read_timeout(Some(Duration::from_secs(5)))
    .is_err()
    || stream
      .set_write_timeout(Some(Duration::from_secs(5)))
      .is_err()
  {
    return;
  }
  let Ok(read_stream) = stream.try_clone() else {
    return;
  };
  let mut reader = BufReader::new(read_stream);
  loop {
    if context.stop.load(Ordering::Relaxed) {
      return;
    }
    let request = match read_request(&mut reader) {
      Ok(Some(request)) => request,
      Ok(None) | Err(ReadError::Io) => return,
      Err(ReadError::Bad) => {
        let _sent = send(stream, &Response::error(400, "bad_request"));
        return;
      }
      Err(ReadError::TooLarge) => {
        let _sent = send(stream, &Response::error(413, "request_too_large"));
        return;
      }
    };
    if respond(stream, context, &request) || request.close {
      return;
    }
  }
}

fn respond(stream: &mut TcpStream, context: &Context, request: &Request) -> bool {
  let local = match stream.local_addr() {
    Ok(local) => local,
    Err(_err) => return true,
  };
  let secrets = context.secrets.current();
  let host_ok = host_allowed(&request.host, local);
  if host_ok && let Some(op) = request.target.strip_prefix(http_action::PREFIX) {
    return act(stream, context, request, op, &secrets);
  }
  let (path, query) = request
    .target
    .split_once('?')
    .unwrap_or((request.target.as_str(), ""));
  let screen_route = path
    .strip_prefix("/api/v1/issues/")
    .and_then(|rest| rest.strip_suffix("/screen"))
    .is_some();
  let response = if !host_ok {
    Some(Response::error(403, "foreign_host"))
  } else if request.method != "GET" && request.method != "HEAD" {
    Some(Response::error(405, "method_not_allowed"))
  } else if context.auth_required
    && !screen_route
    && !authorized(&secrets, request.authorization.as_deref())
  {
    Some(Response::error(401, "unauthorized"))
  } else {
    None
  };
  if let Some(response) = response {
    let mut response = response;
    response.head = request.method == "HEAD";
    let _sent = send(stream, &response);
    return true;
  }
  if path == "/api/v1/events/stream" {
    return if request.method == "GET" {
      crate::http_read::serve_events(stream, context, request, query, &secrets)
    } else {
      crate::http_stream::head(stream, request, query)
    };
  }
  let mut response = crate::http_read::finite_response(context, path, query, request, &secrets);
  response.head = request.method == "HEAD";
  response.close = request.close || response.close;
  let close = response.close;
  let _sent = send(stream, &response);
  close
}

fn act(
  stream: &mut TcpStream,
  context: &Context,
  request: &Request,
  op: &str,
  secrets: &Secrets,
) -> bool {
  let control = Control {
    sender: context.control.as_ref(),
    deadline: context.deadline,
  };
  let mut response = http_action::respond(request, op, secrets, &control);
  response.head = request.method == "HEAD";
  response.close |= request.close;
  let close = response.close;
  let _sent = send(stream, &response);
  close
}

#[cfg(test)]
#[path = "tests/http_test.rs"]
mod tests;
