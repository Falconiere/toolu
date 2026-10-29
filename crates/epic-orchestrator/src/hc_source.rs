//! Streaming hc snapshots and terminal status events.

use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, mpsc::Sender};
use std::time::Duration;

use serde_json::{Value, json};
use toolu_runtime::env::Env;
use toolu_runtime::process::{Spec, run_with_chunks};

use crate::socket_herdr::Signals;
use crate::socket_loop::Msg;
use crate::source::Fact;

mod backoff;

const FRAME_LIMIT: usize = 65_536;

/// Translate one hc JSON frame into engine facts.
fn status(pane: &str, word: &str) -> Fact {
  Fact::Event(
    json!({"event":"pane.agent_status_changed","data":{"pane_id":pane,"agent_status":word}}),
  )
}

fn snapshot_facts(frame: &Value) -> Vec<Fact> {
  let snapshot = crate::hc_backend::snapshot(frame);
  let mut result = vec![Fact::Snapshot(snapshot.clone())];
  if let Some(rows) = snapshot.get("agents").and_then(Value::as_array) {
    for row in rows {
      if let (Some(pane), Some(word)) = (
        row.get("pane_id").and_then(Value::as_str),
        row.get("agent_status").and_then(Value::as_str),
      ) {
        result.push(status(pane, word));
      }
    }
  }
  result
}

/// Translate one hc JSON frame into engine facts.
fn facts(frame: &Value) -> Vec<Fact> {
  let frame = snapshot_frame(frame);
  if frame.get("terminals").and_then(Value::as_array).is_some() {
    return snapshot_facts(frame);
  }
  let Some(event) = frame
    .get("params")
    .filter(|_| frame.get("method").and_then(Value::as_str) == Some("event"))
  else {
    return Vec::new();
  };
  let Some(terminal) = event
    .get("terminal")
    .and_then(Value::as_str)
    .filter(|s| !s.is_empty())
  else {
    return Vec::new();
  };
  let word = match event.get("type").and_then(Value::as_str) {
    Some("created") => {
      let Some(data) = event.get("data") else {
        return Vec::new();
      };
      if data
        .pointer("/labels/toolu.owner")
        .and_then(Value::as_str)
        .is_none()
      {
        return Vec::new();
      }
      let address = data
        .get("address")
        .and_then(Value::as_str)
        .unwrap_or(terminal);
      return vec![status(
        &format!("hc:{address}"),
        toolu_mux::hc::status(data),
      )];
    }
    Some("status") => event
      .pointer("/data/status/state")
      .and_then(Value::as_str)
      .unwrap_or("unknown"),
    Some("finished") => "done",
    _ => return Vec::new(),
  };
  vec![status(&format!("hc:{terminal}"), word)]
}

/// Hydrate summary rows before matching ownership; a failed lookup is not absence.
fn hydrate(frame: &Value, env: &Env) -> Result<Value, ()> {
  let frame = snapshot_frame(frame);
  let Some(rows) = frame.get("terminals").and_then(Value::as_array) else {
    return Ok(frame.clone());
  };
  let client = toolu_mux::hc::Client::new(env);
  let mut terminals = Vec::new();
  for row in rows {
    let address = row
      .get("address")
      .or_else(|| row.get("id"))
      .and_then(Value::as_str)
      .ok_or(())?;
    terminals.push(client.get(address).map_err(|_err| ())?);
  }
  Ok(json!({"terminals":terminals}))
}

fn snapshot_frame(frame: &Value) -> &Value {
  if frame.get("method").and_then(Value::as_str) == Some("missed") {
    frame.get("params").unwrap_or(frame)
  } else {
    frame
  }
}

/// Incrementally decode line-delimited JSON without retaining unbounded output.
#[derive(Default)]
struct Frames {
  bytes: Vec<u8>,
  overflow: bool,
  healthy: bool,
  retry: bool,
}

impl Frames {
  fn push(&mut self, chunk: &[u8], tx: &Sender<Msg>, env: Option<&Env>) {
    for &byte in chunk {
      if byte == b'\n' {
        self.emit(tx, env);
        self.bytes.clear();
        self.overflow = false;
      } else if self.bytes.len() < FRAME_LIMIT {
        self.bytes.push(byte);
      } else {
        self.overflow = true;
      }
    }
  }
  fn emit(&mut self, tx: &Sender<Msg>, env: Option<&Env>) {
    if self.overflow {
      self.retry = true;
      let _sent = tx.send(Msg::Herdr(Fact::HcOutage));
      return;
    }
    let Ok(value) = serde_json::from_slice::<Value>(&self.bytes) else {
      return;
    };
    if let Ok(value) = env.map_or(Ok(value.clone()), |env| hydrate(&value, env)) {
      self.confirm_snapshot(&value, env);
      for fact in facts(&value) {
        let _sent = tx.send(Msg::Herdr(fact));
      }
    } else {
      self.retry = true;
      let _sent = tx.send(Msg::Herdr(Fact::HcOutage));
    }
  }
  fn confirm_snapshot(&mut self, value: &Value, env: Option<&Env>) {
    if value.get("terminals").and_then(Value::as_array).is_none() {
      return;
    }
    self.healthy = true;
    if let Some(env) = env {
      let path = crate::paths::Paths::from_env(env)
        .root
        .join("hc-watch.json");
      backoff::Backoff::default().save(&path);
    }
  }
}

/// Subscribe until shutdown; each reconnect begins with hc's snapshot.
pub(crate) fn run(
  tx: &Sender<Msg>,
  env: &Env,
  signals: &Signals,
  world: &Mutex<crate::model::World>,
) {
  let mut reported_outage = false;
  let path = crate::paths::Paths::from_env(env)
    .root
    .join("hc-watch.json");
  let mut retry = backoff::Backoff::load(&path);
  while !signals.stop.load(Ordering::Relaxed) {
    if !needed(env, world) {
      signals.interrupt.pause(crate::herdr_interrupt::IDLE);
      continue;
    }
    let until = std::time::Instant::now() + retry.remaining();
    while std::time::Instant::now() < until && !signals.stop.load(Ordering::Relaxed) {
      std::thread::sleep(Duration::from_millis(100));
    }
    if signals.stop.load(Ordering::Relaxed) {
      break;
    }
    let healthy = subscribe(tx, env, signals);
    if signals.stop.load(Ordering::Relaxed) {
      break;
    }
    if !reported_outage {
      let _sent = tx.send(Msg::Herdr(Fact::HcOutage));
      reported_outage = true;
    }
    if healthy {
      retry = backoff::Backoff::default();
      reported_outage = false;
    }
    retry.failed(&path);
  }
}

fn needed(env: &Env, world: &Mutex<crate::model::World>) -> bool {
  toolu_mux::backend::configured(env).ok() == Some(toolu_mux::Backend::HerdrApp)
    || world.lock().is_ok_and(|world| {
      world
        .issues
        .values()
        .any(|issue| issue.agent.starts_with("hc:"))
    })
}

fn subscribe(tx: &Sender<Msg>, env: &Env, signals: &Signals) -> bool {
  let frames = Arc::new(Mutex::new(Frames::default()));
  let output = Arc::clone(&frames);
  let sender = tx.clone();
  let event_env = env.clone();
  let interrupt = Arc::clone(&signals.interrupt);
  let observer = Arc::new(move |chunk: &[u8]| {
    if output.lock().is_ok_and(|mut frames| {
      frames.push(chunk, &sender, Some(&event_env));
      frames.retry
    }) {
      interrupt.end_process();
    }
  });
  let mut spec = Spec::new(["hc", "events", "--json"]);
  spec.env = Some(env.clone());
  spec.timeout = Duration::from_secs(3600);
  spec.max_output_bytes = 1;
  let _result = run_with_chunks(
    &spec,
    &mut |pid| {
      signals.interrupt.publish_process(pid);
      Ok(())
    },
    observer,
  );
  signals.interrupt.clear_process();
  frames.lock().is_ok_and(|frames| frames.healthy)
}

#[cfg(test)]
#[path = "tests/hc_source_test.rs"]
mod tests;
