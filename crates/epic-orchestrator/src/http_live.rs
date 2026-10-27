//! `GET /api/v1/agents/live`: the herdr status and terminal title of each
//! worker's agent, from one socket `agent.list` call that every request in
//! the cache window shares. Only HTTP threads call herdr here; the state
//! thread never does.

use std::os::unix::net::UnixStream;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant, SystemTime};

use serde_json::{Value, json};
use toolu_runtime::config::secrets::{Secrets, redact_text};
use toolu_runtime::env::Env;

use crate::herdr::{HERDR_PROTOCOL, roundtrip, socket_path};
use crate::http_response::{Response, json_response};
use crate::model::{World, agent_status_word};

/// How long one `agent.list` reply serves every request.
const FRESH: Duration = Duration::from_secs(2);
/// Herdr calls HTTP threads may run at once.
const HERDR_CALLS: usize = 8;
/// Connect, write and read deadline of each herdr connection.
const TIMEOUT: Duration = Duration::from_secs(1);
const TITLE_CHARS: usize = 160;

/// Where HTTP threads read herdr.
#[derive(Debug, Clone)]
pub(crate) enum Herdr {
  /// `TOOLU_EPIC_HERDR=off`.
  Off,
  /// The engine runs scripted effects (`TOOLU_EPIC_SCRIPTS`).
  Scripted,
  /// The socket `herdr::socket_path` names for this environment.
  Socket(Env),
}

impl Herdr {
  /// The mode of an engine started with `env`.
  pub(crate) fn of(env: &Env) -> Self {
    if env.get("TOOLU_EPIC_SCRIPTS").is_some() {
      Self::Scripted
    } else if crate::source::prompt_enabled() {
      Self::Socket(env.clone())
    } else {
      Self::Off
    }
  }
}

/// Caps herdr calls from HTTP threads; the worker screen route shares it.
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

/// The route's herdr mode, its call limiter, the cached document and the
/// lock that lets one request refill it.
#[derive(Debug)]
pub(crate) struct Live {
  herdr: Herdr,
  limiter: Limiter,
  cached: Mutex<Option<(Instant, Value)>>,
  fill: Mutex<()>,
}

impl Live {
  /// A route that reads herdr as `herdr` says.
  pub(crate) fn new(herdr: Herdr) -> Self {
    Self {
      herdr,
      limiter: Limiter::default(),
      cached: Mutex::new(None),
      fill: Mutex::new(()),
    }
  }

  /// The live document, `503 herdr_busy` when every herdr slot is taken.
  pub(crate) fn response(&self, world: &Mutex<World>, secrets: &Secrets) -> Response {
    let env = match &self.herdr {
      Herdr::Off => return ok(unavailable("off", &now())),
      Herdr::Scripted => return ok(unavailable("scripted", &now())),
      Herdr::Socket(env) => env,
    };
    if let Some(document) = self.fresh() {
      return ok(document);
    }
    // A slot is held while waiting for the fill lock, so overlapping requests
    // count toward the cap even though only one of them calls herdr.
    let Some(_permit) = self.limiter.try_acquire() else {
      return Response::error(503, "herdr_busy");
    };
    let Ok(_fill) = self.fill.lock() else {
      return Response::error(500, "status_unavailable");
    };
    if let Some(document) = self.fresh() {
      return ok(document);
    }
    let Some(document) = sample(env, world, secrets) else {
      return Response::error(500, "status_unavailable");
    };
    if let Ok(mut cached) = self.cached.lock() {
      *cached = Some((Instant::now(), document.clone()));
    }
    ok(document)
  }

  fn fresh(&self) -> Option<Value> {
    let cached = self.cached.lock().ok()?;
    let (at, document) = cached.as_ref()?;
    (at.elapsed() < FRESH).then(|| document.clone())
  }
}

fn ok(document: Value) -> Response {
  json_response(Ok(Some(document)))
}

fn now() -> String {
  toolu_state::time::iso_millis(SystemTime::now())
}

fn unavailable(reason: &str, at: &str) -> Value {
  json!({"schema": 1, "available": false, "reason": reason, "sampled_at": at, "items": []})
}

/// One herdr sample mapped onto the world; `None` when the world is poisoned.
fn sample(env: &Env, world: &Mutex<World>, secrets: &Secrets) -> Option<Value> {
  let listed = agent_list(env);
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
fn agent_list(env: &Env) -> Result<Vec<Value>, String> {
  let pong = call(env, "ping")?;
  let protocol = pong
    .pointer("/result/protocol")
    .and_then(Value::as_u64)
    .unwrap_or(0);
  if protocol != HERDR_PROTOCOL {
    return Err(format!("protocol {protocol}"));
  }
  let reply = call(env, "agent.list")?;
  if let Some(code) = reply.pointer("/error/code").and_then(Value::as_str) {
    return Err(format!("error: {code}"));
  }
  Ok(
    reply
      .pointer("/result/agents")
      .and_then(Value::as_array)
      .cloned()
      .unwrap_or_default(),
  )
}

/// herdr answers one request per connection, so each call opens its own.
fn call(env: &Env, method: &str) -> Result<Value, String> {
  let down = |_err| "not running".to_owned();
  let mut stream = UnixStream::connect(socket_path(env)).map_err(down)?;
  stream.set_read_timeout(Some(TIMEOUT)).map_err(down)?;
  stream.set_write_timeout(Some(TIMEOUT)).map_err(down)?;
  roundtrip(&mut stream, "1", method, &json!({})).map_err(|_err| "not running".to_owned())
}

#[cfg(test)]
#[path = "tests/http_live_test.rs"]
mod tests;
