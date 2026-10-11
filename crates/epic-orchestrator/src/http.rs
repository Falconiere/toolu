//! Bounded TCP listener for read-only epic status.

use std::io::BufReader;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use toolu_runtime::config::secrets::Secrets;

use crate::dashboard::HTML;
use crate::http_access::{authorized, host_allowed};
use crate::http_request::{ReadError, Request, read_request};
use crate::http_response::{OverviewCache, Response, send, stream_headers};
use crate::http_stream;
use crate::model::World;
use crate::paths::Paths;
use crate::status_api;

const CONNECTIONS: usize = 64;
const STREAMS: usize = 8;

/// Bound address and private status credentials.
pub(crate) struct Config {
  pub addr: SocketAddr,
  pub secrets: Secrets,
}

/// A listener thread and its bound address.
pub(crate) struct Running {
  pub addr: SocketAddr,
  pub thread: JoinHandle<()>,
}

struct Context {
  paths: Paths,
  world: Arc<Mutex<World>>,
  changed: Arc<Condvar>,
  stop: Arc<AtomicBool>,
  secrets: Secrets,
  auth_required: bool,
  started: Instant,
  connections: AtomicUsize,
  streams: AtomicUsize,
  overview: OverviewCache,
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
  listener
    .set_nonblocking(true)
    .map_err(|err| format!("HTTP nonblocking: {err}"))?;
  let addr = listener
    .local_addr()
    .map_err(|err| format!("HTTP address: {err}"))?;
  let context = Arc::new(Context {
    paths,
    world,
    changed,
    stop,
    secrets: config.secrets,
    auth_required,
    started: Instant::now(),
    connections: AtomicUsize::new(0),
    streams: AtomicUsize::new(0),
    overview: OverviewCache::new(),
  });
  let thread = thread::spawn(move || accept_loop(&listener, &context));
  Ok(Running { addr, thread })
}

fn accept_loop(listener: &TcpListener, context: &Arc<Context>) {
  while !context.stop.load(Ordering::Relaxed) {
    match listener.accept() {
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
      Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
        thread::sleep(Duration::from_millis(20));
      }
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
  let response = if !host_allowed(&request.host, local) {
    Some(Response::error(403, "foreign_host"))
  } else if request.method != "GET" && request.method != "HEAD" {
    Some(Response::error(405, "method_not_allowed"))
  } else if context.auth_required && !authorized(&context.secrets, request.authorization.as_deref())
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
  let (path, query) = request
    .target
    .split_once('?')
    .unwrap_or((request.target.as_str(), ""));
  if path == "/api/v1/events/stream" {
    return if request.method == "GET" {
      serve_events(stream, context, request, query)
    } else {
      stream_head(stream, request, query)
    };
  }
  let mut response = finite_response(context, path, query, request);
  response.head = request.method == "HEAD";
  response.close = request.close || response.close;
  let close = response.close;
  let _sent = send(stream, &response);
  close
}

fn stream_head(stream: &mut TcpStream, request: &Request, query: &str) -> bool {
  let mut response = if http_stream::cursor(query, request.last_event_id.as_deref()).is_ok() {
    Response {
      status: 200,
      content_type: "text/event-stream; charset=utf-8",
      body: Vec::new(),
      head: true,
      close: request.close,
    }
  } else {
    Response::error(400, "bad_cursor")
  };
  response.head = true;
  let close = response.close;
  let _sent = send(stream, &response);
  close
}

fn finite_response(context: &Context, path: &str, query: &str, request: &Request) -> Response {
  if path == "/" {
    return Response {
      status: 200,
      content_type: "text/html; charset=utf-8",
      body: HTML.as_bytes().to_vec(),
      head: request.method == "HEAD",
      close: false,
    };
  }
  if path == "/api/v1/overview" {
    let world = context.world.lock().unwrap_or_else(PoisonError::into_inner);
    return context
      .overview
      .response(&world, &context.paths, &context.secrets);
  }
  let world = context
    .world
    .lock()
    .unwrap_or_else(PoisonError::into_inner)
    .clone();
  let document = if path == "/api/v1/events" {
    http_stream::page(
      &context.paths,
      query,
      request.last_event_id.as_deref(),
      &context.secrets,
    )
    .map(Some)
  } else {
    status_api::route(
      &world,
      &context.paths,
      path,
      u64::try_from(context.started.elapsed().as_millis()).unwrap_or(u64::MAX),
      &context.secrets,
    )
  };
  match document {
    Ok(Some(value)) => Response {
      status: 200,
      content_type: "application/json; charset=utf-8",
      body: value.to_string().into_bytes(),
      head: false,
      close: false,
    },
    Ok(None) => Response::error(404, "not_found"),
    Err(_err) => Response::error(500, "status_unavailable"),
  }
}

fn serve_events(stream: &mut TcpStream, context: &Context, request: &Request, query: &str) -> bool {
  if context.streams.fetch_add(1, Ordering::AcqRel) >= STREAMS {
    context.streams.fetch_sub(1, Ordering::AcqRel);
    let _sent = send(stream, &Response::error(429, "stream_limit"));
    return true;
  }
  let cursor = http_stream::cursor(query, request.last_event_id.as_deref());
  match cursor {
    Ok(cursor) if request.method == "GET" && stream_headers(stream).is_ok() => {
      http_stream::serve(
        stream,
        cursor,
        &http_stream::Feed {
          paths: &context.paths,
          secrets: &context.secrets,
          world: &context.world,
          changed: &context.changed,
          stop: &context.stop,
        },
      );
    }
    Ok(_) => {}
    Err(_err) => {
      let _sent = send(stream, &Response::error(400, "bad_cursor"));
    }
  }
  context.streams.fetch_sub(1, Ordering::AcqRel);
  true
}

#[cfg(test)]
#[path = "tests/http_test.rs"]
mod tests;
