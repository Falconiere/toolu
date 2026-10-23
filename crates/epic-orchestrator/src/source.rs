//! One herdr session: ping, snapshot and subscribe, each on its own
//! connection, then the events pushed on the subscription.

use std::io::Read;
use std::os::unix::net::UnixStream;
use std::time::Duration;

use serde_json::{Value, json};
use toolu_runtime::env::Env;

use crate::herdr::{self, Call, Client, HERDR_PROTOCOL};
use crate::model::World;
use crate::source_apply;
pub(crate) use crate::source_apply::{Fact, apply};

/// How long a quiet socket waits, and whether quiet ends the call.
pub(crate) struct Pace {
  /// `None` blocks until the peer closes.
  pub idle: Option<Duration>,
  /// Tests return when the socket is quiet. The engine keeps reading.
  pub return_on_idle: bool,
  /// When set, a quiet read returns if the flag is raised.
  pub stop: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
  /// When set, a quiet read reconnects after `pane.created`.
  pub resubscribe: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
  /// When set, the live stream is shared so a stop or resubscribe ends a read.
  pub interrupt: Option<std::sync::Arc<crate::herdr_interrupt::Interrupt>>,
}

enum Stop {
  Idle,
  Retry,
  Done,
}

/// Read one session. `on` runs for each fact before the next read.
///
/// # Errors
/// A journal write or a callback fails. A missing socket is `Fact::Outage`.
pub(crate) fn session<F>(env: &Env, world: &World, pace: &Pace, mut on: F) -> Result<(), String>
where
  F: FnMut(Fact) -> Result<(), String>,
{
  if world.herdr_failures > 0 && world.now_ms < world.herdr_retry_at_ms {
    return Ok(());
  }
  // With an interrupt, the caller reconnects with a fresh world; an
  // in-session retry would reuse stale filters and be shut down at once.
  match connect(env, world, pace, &mut on)? {
    Stop::Retry if !stopped(pace) && pace.interrupt.is_none() => {
      connect(env, world, pace, &mut on).map(|_| ())
    }
    Stop::Retry | Stop::Idle | Stop::Done => Ok(()),
  }
}

fn connect<F>(env: &Env, world: &World, pace: &Pace, on: &mut F) -> Result<Stop, String>
where
  F: FnMut(Fact) -> Result<(), String>,
{
  // herdr answers one request per connection: the ping and the snapshot each
  // get their own, and only the subscription stays open for events.
  let Ok(mut probe) = open(env) else {
    on(Fact::Outage)?;
    return Ok(Stop::Done);
  };
  let protocol = ping(&mut probe)?;
  if protocol > HERDR_PROTOCOL {
    on(Fact::Event(
      json!({"event": "herdr_protocol", "protocol": protocol}),
    ))?;
    return Ok(Stop::Done);
  }
  let snap = round(&mut open(env)?, "2", "session.snapshot", &json!({}))?;
  let body = source_apply::snapshot_body(&snap);
  on(Fact::Snapshot(body.clone()))?;
  let filters = source_apply::filters(world, &body);
  let Some(mut stream) = subscribe(env, &filters) else {
    return Ok(Stop::Retry);
  };
  let Some(interrupt) = &pace.interrupt else {
    return read_events(&mut stream, pace, on);
  };
  interrupt.publish(&stream);
  // A stop that landed before the stream was shared would otherwise wait for
  // a whole idle read.
  let ended = if stopped(pace) {
    Ok(Stop::Done)
  } else {
    read_events(&mut stream, pace, on)
  };
  interrupt.clear();
  ended
}

fn read_events<F>(stream: &mut UnixStream, pace: &Pace, on: &mut F) -> Result<Stop, String>
where
  F: FnMut(Fact) -> Result<(), String>,
{
  if let Some(idle) = pace.idle {
    let _set = stream.set_read_timeout(Some(idle));
  }
  loop {
    match frame(stream) {
      Frame::Line(line) if lost(&line) => return Ok(Stop::Retry),
      Frame::Line(line) if line.get("event").is_some() || line.pointer("/data/type").is_some() => {
        on(Fact::Event(line))?;
      }
      Frame::Idle if pace.return_on_idle => return Ok(Stop::Idle),
      Frame::Idle if stopped(pace) => return Ok(Stop::Done),
      Frame::Idle if take_resubscribe(pace) => return Ok(Stop::Retry),
      Frame::Line(_) | Frame::Idle => {}
      Frame::End => return Ok(Stop::Retry),
    }
  }
}

enum Frame {
  Line(Value),
  Idle,
  End,
}

fn frame(stream: &mut UnixStream) -> Frame {
  let mut buf = Vec::new();
  let mut byte = [0_u8; 1];
  loop {
    if buf.len() >= 65_536 {
      return Frame::End;
    }
    match stream.read(&mut byte) {
      Ok(0) => break,
      Ok(_) if byte[0] == b'\n' => break,
      Ok(_) => buf.push(byte[0]),
      Err(err) if err.kind() == std::io::ErrorKind::TimedOut => return Frame::Idle,
      Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => return Frame::Idle,
      Err(_) => return Frame::End,
    }
  }
  if buf.is_empty() {
    return Frame::End;
  }
  serde_json::from_slice(&buf).map_or(Frame::End, Frame::Line)
}

fn stopped(pace: &Pace) -> bool {
  pace
    .stop
    .as_ref()
    .is_some_and(|flag| flag.load(std::sync::atomic::Ordering::Relaxed))
}

fn take_resubscribe(pace: &Pace) -> bool {
  pace
    .resubscribe
    .as_ref()
    .is_some_and(|flag| flag.swap(false, std::sync::atomic::Ordering::Relaxed))
}

fn lost(line: &Value) -> bool {
  line.pointer("/error/code").and_then(Value::as_str) == Some("events_lost")
}

fn ping(stream: &mut UnixStream) -> Result<u64, String> {
  let reply = round(stream, "1", "ping", &json!({}))?;
  Ok(
    reply
      .pointer("/result/protocol")
      .and_then(Value::as_u64)
      .unwrap_or(0),
  )
}

/// Send `text` to `target` when the engine has no script directory.
///
/// # Errors
/// The herdr socket or the CLI prompt fails.
pub(crate) fn prompt(
  paths: &crate::paths::Paths,
  env: &Env,
  target: &str,
  text: &str,
) -> Result<(), String> {
  let mut client = Client::new();
  client
    .execute(
      paths,
      env,
      &Call::Prompt {
        target: target.to_owned(),
        text: text.to_owned(),
      },
    )
    .and_then(|reply| crate::herdr_calls::delivered("agent.prompt", &reply))
}

/// The binary subscribes unless `TOOLU_EPIC_HERDR=off`. Tests subscribe only when it is `1`.
pub(crate) fn prompt_enabled() -> bool {
  let env = Env::process();
  if env.get("TOOLU_EPIC_HERDR") == Some("off") {
    return false;
  }
  if cfg!(test) {
    return env.get("TOOLU_EPIC_HERDR") == Some("1");
  }
  true
}

/// A new connection subscribed to `filters`, or `None` when herdr cannot be
/// reached, drops the request or answers with an error.
fn subscribe(env: &Env, filters: &[Value]) -> Option<UnixStream> {
  let mut stream = open(env).ok()?;
  let reply = round(
    &mut stream,
    "3",
    "events.subscribe",
    &json!({"subscriptions": filters}),
  )
  .ok()?;
  reply.get("error").is_none().then_some(stream)
}

fn open(env: &Env) -> Result<UnixStream, String> {
  UnixStream::connect(herdr::socket_path(env)).map_err(|err| err.to_string())
}

fn round(stream: &mut UnixStream, id: &str, method: &str, params: &Value) -> Result<Value, String> {
  herdr::roundtrip(stream, id, method, params)
}

#[cfg(test)]
#[path = "tests/source_test.rs"]
mod tests;
