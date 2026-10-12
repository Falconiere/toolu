//! Socket operations: dispatch one request, ingest the spool, wake waiters.

use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::journal::{self, Record};
use crate::model::Report;
use crate::schedule::note_event;
use crate::server::{Engine, Stop};
use crate::socket_loop::Waiter;
use crate::status::wait_body;
use crate::status_api::document;

/// Apply one control-socket request and send its response.
pub(crate) fn dispatch(
  engine: &mut Engine,
  request: &Value,
  reply: &Sender<Value>,
  waiters: &mut Vec<Waiter>,
  protocol: u64,
) -> bool {
  match operate(engine, request, reply.clone(), waiters, protocol) {
    Ok(stop) => stop,
    Err(err) => {
      let _sent = reply.send(json!({"error": err}));
      false
    }
  }
}

fn operate(
  engine: &mut Engine,
  request: &Value,
  reply: Sender<Value>,
  waiters: &mut Vec<Waiter>,
  protocol: u64,
) -> Result<bool, String> {
  let op = request.get("op").and_then(Value::as_str).unwrap_or("");
  let client = request
    .get("protocol")
    .and_then(Value::as_u64)
    .unwrap_or(protocol);
  if op != "replace" && client != protocol {
    let _sent = reply.send(json!({"error": "protocol"}));
    return Ok(false);
  }
  match op {
    "stop" | "replace" => finish(engine, &reply, op == "replace"),
    "status" => reply_status(engine, request, &reply),
    "queue" => {
      let epic = request.get("epic").and_then(Value::as_str);
      let _sent = reply.send(crate::queue::document(&engine.world, epic));
      Ok(false)
    }
    "pause" => reply_pause(engine, request, &reply, true),
    "resume" => reply_pause(engine, request, &reply, false),
    "ack" => crate::attention_ops::reply_ack(engine, request, &reply),
    "event" => reply_event(engine, request, &reply),
    "report" => reply_report(engine, request, &reply, waiters),
    "wait" => Ok(reply_wait(engine, request, reply, waiters)),
    "answer" => crate::attention_ops::reply_answer(engine, request, &reply),
    "release" => crate::dispatch_ops::reply_release(engine, request, &reply),
    _ => {
      let _sent = reply.send(json!({"error": "unknown op"}));
      Ok(false)
    }
  }
}

fn finish(engine: &mut Engine, reply: &Sender<Value>, draining: bool) -> Result<bool, String> {
  // Both stop issuing calls at once; the loop exits once the running call returns.
  engine.world.draining = true;
  if draining {
    let _stop = engine.pump()?;
  }
  let _sent = reply.send(json!({"ok": true}));
  Ok(true)
}

fn reply_status(engine: &Engine, request: &Value, reply: &Sender<Value>) -> Result<bool, String> {
  let epic = request.get("epic").and_then(Value::as_str);
  let roots = toolu_runtime::host::roots::Roots::new(toolu_runtime::env::Env::process(), None);
  let secrets = toolu_runtime::config::secrets::load(&roots).map_err(|err| err.to_string())?;
  let doc = document(&engine.world, &engine.paths, epic, &secrets)?;
  let _sent = reply.send(doc);
  Ok(false)
}

fn reply_pause(
  engine: &mut Engine,
  request: &Value,
  reply: &Sender<Value>,
  pause: bool,
) -> Result<bool, String> {
  let epic = request.get("epic").and_then(Value::as_str);
  let result = if pause {
    engine.pause(epic)
  } else {
    engine.resume(epic)
  };
  let _sent = reply.send(result_value(result));
  if pause { Ok(false) } else { pump_after(engine) }
}

fn reply_event(
  engine: &mut Engine,
  request: &Value,
  reply: &Sender<Value>,
) -> Result<bool, String> {
  let key = request.get("key").and_then(Value::as_str).unwrap_or("");
  let event = request.get("event").and_then(Value::as_str).unwrap_or("");
  note_event(&mut engine.world, key, event);
  engine.flush()?;
  let _sent = reply.send(json!({"ok": true}));
  pump_after(engine)
}

fn reply_report(
  engine: &mut Engine,
  request: &Value,
  reply: &Sender<Value>,
  waiters: &mut Vec<Waiter>,
) -> Result<bool, String> {
  if let Some(token) = request.get("token").and_then(Value::as_str)
    && !token.is_empty()
    && spool_seen(engine, token)?
  {
    let _sent = reply.send(json!({"ok": true}));
    return Ok(false);
  }
  engine.report(&report_from(request))?;
  remember_spool(engine, request)?;
  satisfy(engine, waiters);
  let _sent = reply.send(json!({"ok": true}));
  pump_after(engine)
}

pub(crate) fn pump_after(engine: &mut Engine) -> Result<bool, String> {
  Ok(engine.pump()? == Stop::Fault)
}

fn reply_wait(
  engine: &mut Engine,
  request: &Value,
  reply: Sender<Value>,
  waiters: &mut Vec<Waiter>,
) -> bool {
  let max_seconds = request
    .get("max_seconds")
    .and_then(Value::as_u64)
    .unwrap_or(0);
  if let Some(body) = ready_wait(&mut engine.world, max_seconds) {
    let _sent = reply.send(body);
    drop(reply);
    return false;
  }
  let now = Instant::now();
  waiters.push(Waiter {
    deadline: now
      .checked_add(Duration::from_secs(max_seconds))
      .unwrap_or(now),
    reply,
  });
  false
}

fn ready_wait(world: &mut crate::model::World, max_seconds: u64) -> Option<Value> {
  let body = wait_body(world, max_seconds);
  (max_seconds == 0 || body.get("kind").is_some()).then_some(body)
}

/// `{"ok":true}`, or `{"error":…}`.
pub(crate) fn result_value(result: Result<(), String>) -> Value {
  match result {
    Ok(()) => json!({"ok": true}),
    Err(err) => json!({"error": err}),
  }
}

fn report_from(request: &Value) -> Report {
  Report {
    key: request
      .get("key")
      .and_then(Value::as_str)
      .unwrap_or("")
      .to_owned(),
    epic: request
      .get("epic")
      .and_then(Value::as_str)
      .unwrap_or("")
      .to_owned(),
    state_dir: request
      .get("state_dir")
      .and_then(Value::as_str)
      .unwrap_or("")
      .to_owned(),
    phase: request
      .get("phase")
      .and_then(Value::as_str)
      .unwrap_or("")
      .to_owned(),
    pr: request.get("pr").and_then(Value::as_u64),
    note: request
      .get("note")
      .and_then(Value::as_str)
      .unwrap_or("")
      .to_owned(),
  }
}

/// Reply to waiters whose judgment has become available.
pub(crate) fn satisfy(engine: &mut Engine, waiters: &mut Vec<Waiter>) {
  if waiters.is_empty() {
    return;
  }
  let body = wait_body(&mut engine.world, 1);
  if body.get("kind").is_some() {
    let reply = waiters.remove(0).reply;
    let _sent = reply.send(body);
    return;
  }
  let now = Instant::now();
  let mut staying = Vec::new();
  for waiter in waiters.drain(..) {
    if waiter.deadline <= now {
      let _sent = waiter.reply.send(json!({"state": "waiting"}));
    } else {
      staying.push(waiter);
    }
  }
  *waiters = staying;
}

/// Reply to waiters whose deadlines have passed.
pub(crate) fn expire(engine: &mut Engine, waiters: &mut Vec<Waiter>) {
  satisfy(engine, waiters);
}

fn remember_spool(engine: &Engine, request: &Value) -> Result<(), String> {
  let Some(token) = request.get("token").and_then(Value::as_str) else {
    return Ok(());
  };
  let key = request.get("key").and_then(Value::as_str).unwrap_or("");
  let record = Record::new("action", "spool", key, token, "");
  journal::append(
    &engine.paths.journal_dir(),
    &engine.paths.journal_lock(),
    &record,
    engine.now,
  )?;
  let path = crate::client::spool_path(&engine.paths.root, token)?;
  if path.is_file() {
    std::fs::remove_file(path).map_err(|err| err.to_string())?;
  }
  Ok(())
}

/// Ingest reports spooled while the engine was unavailable.
pub(crate) fn ingest(engine: &mut Engine) -> Result<(), String> {
  let mut files = spool_files(&engine.paths.spool())?;
  files.sort();
  for path in files {
    ingest_one(engine, &path)?;
  }
  Ok(())
}

fn spool_files(dir: &std::path::Path) -> Result<Vec<std::path::PathBuf>, String> {
  let entries = match std::fs::read_dir(dir) {
    Ok(entries) => entries,
    Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
    Err(err) => return Err(err.to_string()),
  };
  let mut files = Vec::new();
  for entry in entries {
    files.push(entry.map_err(|err| err.to_string())?.path());
  }
  Ok(files)
}

fn ingest_one(engine: &mut Engine, path: &std::path::Path) -> Result<(), String> {
  let Ok(value) = crate::disk::read_value(path) else {
    let _quarantined = std::fs::rename(path, path.with_extension("bad"));
    return Ok(());
  };
  let token = value.get("token").and_then(Value::as_str).unwrap_or("");
  if token.is_empty() || spool_seen(engine, token)? {
    let _gone = std::fs::remove_file(path);
    return Ok(());
  }
  engine.report(&report_from(&value))?;
  remember_spool(engine, &value)
}

fn spool_seen(engine: &Engine, token: &str) -> Result<bool, String> {
  let rows = journal::tail(&engine.paths.journal_dir(), engine.now)?;
  Ok(
    rows
      .iter()
      .any(|row| row.name == "spool" && row.token == token),
  )
}

#[cfg(test)]
#[path = "tests/dispatch_test.rs"]
mod tests;
