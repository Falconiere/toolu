//! Map a herdr snapshot or event onto the engine.

use serde_json::Value;
use toolu_runtime::env::Env;

use crate::herdr::{Call, Client};
use crate::herdr_calls::request;
use crate::journal::Record;
use crate::limit::on_status;
use crate::logic::push_attention;
mod identity;
use crate::schedule::{map_pane_closed, note_event, note_herdr};
use crate::server::Engine;
use identity::{event_key, event_kind, field, same_agent};
pub(crate) use identity::{filters, snapshot_body};

/// A fact the subscription thread hands the state thread.
#[derive(Debug, Clone)]
pub(crate) enum Fact {
  /// `session.snapshot` body.
  Snapshot(Value),
  /// One pushed event.
  Event(Value),
  /// The socket could not be reached.
  Outage,
  /// The hc event stream could not be reached.
  HcOutage,
}

/// Apply one fact and journal it.
///
/// # Errors
/// The journal or a cooldown file cannot be written.
pub(crate) fn apply(engine: &mut Engine, env: &Env, fact: Fact) -> Result<(), String> {
  match fact {
    Fact::Outage => outage(engine, false),
    Fact::HcOutage => outage(engine, true),
    Fact::Snapshot(body) => apply_snapshot(engine, &body),
    Fact::Event(line) => apply_event(engine, env, &line),
  }
}

fn apply_snapshot(engine: &mut Engine, body: &Value) -> Result<(), String> {
  let hc = body.get("backend").and_then(Value::as_str) == Some("herdr-app");
  if !hc {
    note_herdr(&mut engine.world, true);
    engine.save_watch()?;
  }
  journal(engine, "herdr-snapshot", "", "session.snapshot");
  let rows = body.get("agents").and_then(Value::as_array);
  let keys: Vec<String> = engine.world.issues.keys().cloned().collect();
  for key in &keys {
    if engine
      .world
      .issues
      .get(key)
      .is_some_and(|issue| issue.agent.starts_with("hc:") == hc)
    {
      bind_or_gone(engine, key, rows);
    }
  }
  engine.pump().map(|_| ())
}

fn bind_or_gone(engine: &mut Engine, key: &str, rows: Option<&Vec<Value>>) {
  let Some(issue) = engine.world.issues.get(key) else {
    return;
  };
  let running = issue.stage == "running";
  let found = rows.and_then(|rows| rows.iter().find(|row| same_agent(issue, row)));
  if let Some(row) = found {
    let pane = row
      .get("pane_id")
      .and_then(Value::as_str)
      .map(str::to_owned);
    let status = row.get("agent_status").and_then(Value::as_str);
    let now = engine.world.now_ms;
    if let Some(issue) = engine.world.issues.get_mut(key) {
      issue.set_agent_status(status.unwrap_or("unknown"), now);
      if pane.is_some() {
        issue.pane = pane;
      }
    }
    return;
  }
  // An agent herdr does not list cannot be working in the worktree.
  let now = engine.world.now_ms;
  if let Some(issue) = engine.world.issues.get_mut(key) {
    issue.set_agent_status("unknown", now);
  }
  if running {
    if engine
      .world
      .issues
      .get(key)
      .is_some_and(|issue| issue.agent.starts_with("hc:"))
    {
      push_attention(
        &mut engine.world,
        "needs-human",
        key,
        "hc terminal missing; inspect before recovery",
      );
    } else {
      note_event(&mut engine.world, key, "gone");
    }
  }
}

fn apply_event(engine: &mut Engine, env: &Env, line: &Value) -> Result<(), String> {
  let kind = event_kind(line);
  let key = event_key(engine, line, &kind);
  if kind == "herdr_protocol" {
    let note = protocol_note(line);
    journal(engine, "herdr-protocol", "", &note);
  } else {
    journal(engine, "herdr-event", key.as_deref().unwrap_or(""), &kind);
  }
  match kind.as_str() {
    "pane_closed" | "pane_exited" => {
      // Its agent is gone, so no status event will follow for that pane.
      let now = engine.world.now_ms;
      if let Some(issue) = key
        .as_ref()
        .and_then(|key| engine.world.issues.get_mut(key))
      {
        issue.set_agent_status("unknown", now);
      }
      if let Some(pane) = field(line, "pane_id") {
        map_pane_closed(&mut engine.world, pane);
      }
    }
    "worktree_removed" => close_worktree(engine, line),
    "pane_agent_status_changed" => status_event(engine, env, line)?,
    "pane_created" => created(engine, line),
    _ => {}
  }
  engine.flush()?;
  engine.pump().map(|_| ())
}

fn protocol_note(line: &Value) -> String {
  line.get("protocol").and_then(Value::as_u64).map_or_else(
    || "protocol".to_owned(),
    |protocol| format!("protocol {protocol}"),
  )
}

fn status_event(engine: &mut Engine, env: &Env, line: &Value) -> Result<(), String> {
  let Some(pane) = field(line, "pane_id") else {
    return Ok(());
  };
  let Some(key) = engine
    .world
    .issues
    .iter()
    .find(|(_, issue)| issue.pane.as_deref() == Some(pane) || issue.agent == pane)
    .map(|(key, _)| key.clone())
  else {
    return Ok(());
  };
  let status = field(line, "agent_status").unwrap_or("unknown");
  let now = engine.world.now_ms;
  if let Some(issue) = engine.world.issues.get_mut(&key) {
    issue.set_agent_status(status, now);
  }
  if status == "working" || (status != "blocked" && scan_done(engine, &key, status)) {
    return on_status(engine, &key, status, "");
  }
  on_status(engine, &key, status, &read_tail(engine, env, &key))
}

fn scan_done(engine: &Engine, key: &str, status: &str) -> bool {
  if !matches!(status, "idle" | "done" | "blocked") {
    return false;
  }
  engine.world.issues.get(key).is_some_and(|issue| {
    matches!(issue.phase.as_str(), "ready" | "needs-human")
      || issue.scanned_at == Some(issue.phase_at_ms)
  })
}

fn read_tail(engine: &Engine, env: &Env, key: &str) -> String {
  let target = engine
    .world
    .issues
    .get(key)
    .map_or_else(|| key.to_owned(), |issue| issue.agent.clone());
  let mut client = Client::new();
  request(&mut client, &engine.paths, env, &Call::Read { target })
    .ok()
    .and_then(|reply| {
      let text = reply.pointer("/read/text").and_then(Value::as_str);
      text.map(str::to_owned)
    })
    .unwrap_or_default()
}

fn created(engine: &mut Engine, line: &Value) {
  let cwd = field(line, "cwd").or_else(|| field(line, "path"));
  let pane = field(line, "pane_id").map(str::to_owned);
  let Some(cwd) = cwd else {
    return;
  };
  let key = engine.world.issues.iter().find_map(|(key, issue)| {
    let hit = issue.stage == "running" && issue.worktree.as_deref() == Some(cwd);
    hit.then(|| key.clone())
  });
  let Some(key) = key else {
    return;
  };
  if let Some(issue) = engine.world.issues.get_mut(&key) {
    issue.pane = pane;
  }
  engine.resubscribe = true;
}

fn close_worktree(engine: &mut Engine, line: &Value) {
  let path = line
    .pointer("/data/worktree/path")
    .and_then(Value::as_str)
    .or_else(|| field(line, "path"));
  let Some(path) = path else {
    return;
  };
  let key = engine.world.issues.iter().find_map(|(key, issue)| {
    let hit = issue.stage == "running" && issue.worktree.as_deref() == Some(path);
    hit.then(|| key.clone())
  });
  if let Some(key) = key {
    note_event(&mut engine.world, &key, "gone");
  }
}

fn outage(engine: &mut Engine, hc: bool) -> Result<(), String> {
  if hc {
    let now = engine.world.now_ms;
    for issue in engine
      .world
      .issues
      .values_mut()
      .filter(|issue| issue.agent.starts_with("hc:"))
    {
      issue.set_agent_status("unknown", now);
    }
    return Ok(());
  }
  let first = engine.world.herdr_failures == 0;
  let running = engine
    .world
    .issues
    .iter()
    .find(|(_, issue)| issue.stage == "running" && !issue.agent.starts_with("hc:"))
    .map(|(key, _)| key.clone());
  note_herdr(&mut engine.world, false);
  let now = engine.world.now_ms;
  for issue in engine
    .world
    .issues
    .values_mut()
    .filter(|issue| !issue.agent.starts_with("hc:"))
  {
    issue.set_agent_status("unknown", now);
  }
  engine.save_watch()?;
  if first && let Some(key) = running {
    push_attention(&mut engine.world, "herdr-error", &key, "herdr unreachable");
    engine.flush()?;
  }
  Ok(())
}

fn journal(engine: &mut Engine, name: &str, key: &str, note: &str) {
  engine
    .world
    .outbox
    .push_back(crate::model::Step::Journal(Record::new(
      "action", name, key, "", note,
    )));
}

#[cfg(test)]
#[path = "tests/source_apply_test.rs"]
mod tests;
