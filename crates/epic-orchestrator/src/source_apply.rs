//! Map a herdr snapshot or event onto the engine.

use serde_json::Value;
use toolu_runtime::env::Env;

use crate::herdr::{Call, Client};
use crate::journal::Record;
use crate::limit::on_status;
use crate::logic::push_attention;
use crate::model::World;
use crate::schedule::{map_pane_closed, note_event, note_herdr};
use crate::server::Engine;

/// A fact the subscription thread hands the state thread.
#[derive(Debug, Clone)]
pub(crate) enum Fact {
  /// `session.snapshot` body.
  Snapshot(Value),
  /// One pushed event.
  Event(Value),
  /// The socket could not be reached.
  Outage,
}

/// Apply one fact and journal it.
///
/// # Errors
/// The journal or a cooldown file cannot be written.
pub(crate) fn apply(engine: &mut Engine, env: &Env, fact: Fact) -> Result<(), String> {
  match fact {
    Fact::Outage => outage(engine),
    Fact::Snapshot(body) => apply_snapshot(engine, &body),
    Fact::Event(line) => apply_event(engine, env, &line),
  }
}

fn apply_snapshot(engine: &mut Engine, body: &Value) -> Result<(), String> {
  note_herdr(&mut engine.world, true);
  engine.save_watch()?;
  journal(engine, "herdr-snapshot", "", "session.snapshot");
  let rows = body.get("agents").and_then(Value::as_array);
  let keys: Vec<String> = engine.world.issues.keys().cloned().collect();
  for key in &keys {
    bind_or_gone(engine, key, rows);
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
    if pane.is_some()
      && let Some(issue) = engine.world.issues.get_mut(key)
    {
      issue.pane = pane;
    }
    return;
  }
  if running {
    note_event(&mut engine.world, key, "gone");
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
    .find(|(_, issue)| issue.pane.as_deref() == Some(pane))
    .map(|(key, _)| key.clone())
  else {
    return Ok(());
  };
  let status = field(line, "agent_status").unwrap_or("unknown");
  if status == "working" || scan_done(engine, &key, status) {
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
  client
    .execute(&engine.paths, env, &Call::Read { target })
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

fn outage(engine: &mut Engine) -> Result<(), String> {
  let first = engine.world.herdr_failures == 0;
  let running = engine
    .world
    .issues
    .iter()
    .find(|(_, issue)| issue.stage == "running")
    .map(|(key, _)| key.clone());
  note_herdr(&mut engine.world, false);
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

fn same_agent(issue: &crate::model::Issue, row: &Value) -> bool {
  let name = row.get("name").and_then(Value::as_str);
  if name.is_some_and(|name| name == issue.agent) {
    return true;
  }
  let tree = issue.worktree.as_deref();
  let cwd = row.get("cwd").and_then(Value::as_str);
  let fg = row.get("foreground_cwd").and_then(Value::as_str);
  if tree.is_some() && (cwd == tree || fg == tree) {
    return true;
  }
  let pane = row.get("pane_id").and_then(Value::as_str);
  pane.is_some() && pane == issue.pane.as_deref()
}

pub(crate) fn filters(world: &World, body: &Value) -> Vec<Value> {
  let mut types = vec![
    serde_json::json!({"type": "pane.closed"}),
    serde_json::json!({"type": "pane.exited"}),
    serde_json::json!({"type": "worktree.removed"}),
    serde_json::json!({"type": "pane.created"}),
  ];
  let Some(rows) = body.get("agents").and_then(Value::as_array) else {
    return types;
  };
  for row in rows {
    let pane = row.get("pane_id").and_then(Value::as_str);
    let mapped = world.issues.values().any(|issue| same_agent(issue, row));
    if mapped && let Some(pane) = pane {
      types.push(serde_json::json!({"type": "pane.agent_status_changed", "pane_id": pane}));
    }
  }
  types
}

pub(crate) fn snapshot_body(reply: &Value) -> Value {
  reply
    .pointer("/result/snapshot")
    .cloned()
    .or_else(|| reply.get("snapshot").cloned())
    .unwrap_or_else(|| reply.clone())
}

fn event_kind(line: &Value) -> String {
  let raw = line
    .get("event")
    .and_then(Value::as_str)
    .or_else(|| event_type(line))
    .unwrap_or("");
  raw.replace('.', "_")
}

fn event_type(line: &Value) -> Option<&str> {
  line.pointer("/data/type").and_then(Value::as_str)
}

fn event_key(engine: &Engine, line: &Value, kind: &str) -> Option<String> {
  if kind == "worktree_removed" {
    let path = line
      .pointer("/data/worktree/path")
      .and_then(Value::as_str)?;
    return engine
      .world
      .issues
      .iter()
      .find_map(|(key, issue)| (issue.worktree.as_deref() == Some(path)).then(|| key.clone()));
  }
  let pane = field(line, "pane_id")?;
  engine
    .world
    .issues
    .iter()
    .find(|(_, issue)| issue.pane.as_deref() == Some(pane))
    .map(|(key, _)| key.clone())
}

fn field<'a>(line: &'a Value, key: &str) -> Option<&'a str> {
  line
    .pointer(&format!("/data/{key}"))
    .and_then(Value::as_str)
    .or_else(|| line.get(key).and_then(Value::as_str))
}

#[cfg(test)]
#[path = "tests/source_apply_test.rs"]
mod tests;
