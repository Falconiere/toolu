//! `GET /api/v1/agents/live`: the herdr status and terminal title of each
//! worker's agent, from one socket `agent.list` call that every request in
//! the cache window shares. Only HTTP threads call herdr here; the state
//! thread never does.

use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, PoisonError, mpsc};
use std::time::{Duration, Instant, SystemTime};

use serde_json::{Value, json};
use toolu_runtime::config::secrets::{Secrets, redact_text};
use toolu_runtime::env::Env;

use crate::client::{read_text, write_json};
use crate::herdr::{HERDR_PROTOCOL, roundtrip, socket_path};
use crate::http_response::Response;
use crate::model::{World, agent_status_word};

/// How long one `agent.list` reply serves every request.
const FRESH: Duration = Duration::from_secs(2);
/// Herdr calls HTTP threads may run at once.
const HERDR_CALLS: usize = 8;
/// Connect, write and read timeout of each herdr connection.
const TIMEOUT: Duration = Duration::from_secs(1);
const TITLE_CHARS: usize = 160;

/// Where HTTP threads read herdr.
#[derive(Debug, Clone)]
pub(crate) enum Herdr {
  /// `TOOLU_EPIC_HERDR=off`.
  Off,
  /// The engine runs scripted effects (`TOOLU_EPIC_SCRIPTS`).
  Scripted,
  /// The herdr socket at this path.
  Socket(PathBuf),
}

impl Herdr {
  /// The mode of an engine started with `env`.
  pub(crate) fn of(env: &Env) -> Self {
    if env.get("TOOLU_EPIC_SCRIPTS").is_some() {
      Self::Scripted
    } else if crate::source::prompt_enabled() {
      Self::Socket(socket_path(env))
    } else {
      Self::Off
    }
  }
}

/// Caps herdr calls from HTTP threads; the listener owns one for every route
/// that calls herdr.
#[derive(Debug, Default)]
pub(crate) struct Limiter {
  busy: AtomicUsize,
}

/// One held herdr call slot, freed on drop.
pub(crate) struct Permit<'a>(&'a AtomicUsize);

impl Limiter {
  /// A slot, or `None` when every slot is taken.
  pub(crate) fn try_acquire(&self) -> Option<Permit<'_>> {
    self
      .busy
      .try_update(Ordering::AcqRel, Ordering::Acquire, |busy| {
        (busy < HERDR_CALLS).then_some(busy.saturating_add(1))
      })
      .ok()
      .map(|_| Permit(&self.busy))
  }
}

impl Drop for Permit<'_> {
  fn drop(&mut self) {
    self.0.fetch_sub(1, Ordering::AcqRel);
  }
}

/// The route's herdr mode, the cached JSON body and the lock that lets one
/// request refill it.
#[derive(Debug)]
pub(crate) struct Live {
  herdr: Herdr,
  cached: Mutex<Option<(Instant, Vec<u8>)>>,
  fill: Mutex<()>,
}

impl Live {
  /// A route that reads herdr as `herdr` says.
  pub(crate) fn new(herdr: Herdr) -> Self {
    Self {
      herdr,
      cached: Mutex::new(None),
      fill: Mutex::new(()),
    }
  }

  /// The live document, `503 herdr_busy` when every herdr slot is taken.
  pub(crate) fn response(
    &self,
    limiter: &Limiter,
    world: &Mutex<World>,
    secrets: &Secrets,
  ) -> Response {
    let socket = match &self.herdr {
      Herdr::Off => return ok(unavailable("off", &now()).to_string().into_bytes()),
      Herdr::Scripted => return ok(unavailable("scripted", &now()).to_string().into_bytes()),
      Herdr::Socket(socket) => socket,
    };
    if let Some(body) = self.fresh() {
      return ok(body);
    }
    // A slot is held while waiting for the fill lock, so overlapping requests
    // count toward the cap even though only one of them calls herdr.
    let Some(_permit) = limiter.try_acquire() else {
      return Response::error(503, "herdr_busy");
    };
    // `fill` guards no data, so a panicked filler must not wedge the route.
    let _fill = self.fill.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(body) = self.fresh() {
      return ok(body);
    }
    let Some(document) = sample(socket, world, secrets) else {
      return Response::error(500, "status_unavailable");
    };
    let body = document.to_string().into_bytes();
    if let Ok(mut cached) = self.cached.lock() {
      *cached = Some((Instant::now(), body.clone()));
    }
    ok(body)
  }

  fn fresh(&self) -> Option<Vec<u8>> {
    let cached = self.cached.lock().ok()?;
    let (at, body) = cached.as_ref()?;
    (at.elapsed() < FRESH).then(|| body.clone())
  }
}

fn ok(body: Vec<u8>) -> Response {
  Response {
    status: 200,
    content_type: "application/json; charset=utf-8",
    body,
    head: false,
    close: false,
  }
}

fn now() -> String {
  toolu_state::time::iso_millis(SystemTime::now())
}

fn unavailable(reason: &str, at: &str) -> Value {
  json!({"schema": 1, "available": false, "reason": reason, "sampled_at": at, "items": []})
}

/// One herdr sample mapped onto the world; `None` when the world is poisoned.
fn sample(socket: &Path, world: &Mutex<World>, secrets: &Secrets) -> Option<Value> {
  let listed = agent_list(socket);
  let at = now();
  let rows = match listed {
    Ok(rows) => rows,
    Err(reason) => return Some(unavailable(&reason, &at)),
  };
  let world = world.lock().ok()?;
  let items: Vec<Value> = world
    .issues
    .values()
    .filter_map(|issue| {
      let row = rows
        .iter()
        .find(|row| row.get("name").and_then(Value::as_str) == Some(issue.agent.as_str()))?;
      let status = row.get("agent_status").and_then(Value::as_str);
      Some(json!({
        "key": issue.key,
        "epic": issue.epic,
        "host": (!issue.kind.is_empty()).then_some(&issue.kind),
        "agent_status": agent_status_word(status.unwrap_or("unknown")),
        "title": title(row, secrets),
        "sampled_at": at,
      }))
    })
    .collect();
  Some(json!({"schema": 1, "available": true, "sampled_at": at, "items": items}))
}

/// herdr's stripped terminal title, redacted before it is cut so a cut
/// never leaves part of a secret.
fn title(row: &Value, secrets: &Secrets) -> Option<String> {
  let raw = row
    .get("terminal_title_stripped")
    .and_then(Value::as_str)
    .filter(|title| !title.is_empty())?;
  Some(
    redact_text(raw, secrets)
      .chars()
      .take(TITLE_CHARS)
      .collect(),
  )
}

/// `agent.list` rows over the socket at protocol 22 only: a CLI fallback
/// would spawn a process and journal a record on every poll.
fn agent_list(socket: &Path) -> Result<Vec<Value>, String> {
  let pong = roundtrip(&mut connect(socket)?, "1", "ping", &json!({})).map_err(down)?;
  failed(&pong)?;
  let protocol = pong.pointer("/result/protocol").and_then(Value::as_u64);
  if protocol != Some(HERDR_PROTOCOL) {
    return Err(format!("protocol {}", protocol.unwrap_or(0)));
  }
  // herdr answers one request per connection; nothing follows the reply, so
  // it is read in blocks rather than a byte at a time.
  let mut stream = connect(socket)?;
  write_json(
    &mut stream,
    &json!({"id": "2", "method": "agent.list", "params": {}}),
  )
  .map_err(down)?;
  let reply: Value = serde_json::from_str(&read_text(&mut stream).map_err(down)?).map_err(down)?;
  failed(&reply)?;
  Ok(
    reply
      .pointer("/result/agents")
      .and_then(Value::as_array)
      .cloned()
      .unwrap_or_default(),
  )
}

/// `error: <code>` for a herdr error object.
fn failed(reply: &Value) -> Result<(), String> {
  match reply.get("error").filter(|error| error.is_object()) {
    Some(error) => Err(format!(
      "error: {}",
      error.get("code").and_then(Value::as_str).unwrap_or("null")
    )),
    None => Ok(()),
  }
}

/// A connection within [`TIMEOUT`]. A Unix connect blocks while the server's
/// backlog is full, so it runs on its own thread, which ends with the connect.
fn connect(socket: &Path) -> Result<UnixStream, String> {
  let (sent, received) = mpsc::channel();
  let path = socket.to_path_buf();
  std::thread::spawn(move || sent.send(UnixStream::connect(path)));
  let stream = received
    .recv_timeout(TIMEOUT)
    .map_err(down)?
    .map_err(down)?;
  stream.set_read_timeout(Some(TIMEOUT)).map_err(down)?;
  stream.set_write_timeout(Some(TIMEOUT)).map_err(down)?;
  Ok(stream)
}

fn down<E>(_err: E) -> String {
  "not running".to_owned()
}

#[cfg(test)]
#[path = "tests/http_live_test.rs"]
mod tests;
