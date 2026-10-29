//! Snapshot and event identity mapping.

use crate::model::World;
use crate::server::Engine;
use serde_json::Value;

pub(super) fn same_agent(issue: &crate::model::Issue, row: &Value) -> bool {
  let name = row.get("name").and_then(Value::as_str);
  if issue.agent.starts_with("hc:") {
    return name == Some(issue.agent.as_str());
  }
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

pub(super) fn event_kind(line: &Value) -> String {
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

pub(super) fn event_key(engine: &Engine, line: &Value, kind: &str) -> Option<String> {
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
    .find(|(_, issue)| issue.pane.as_deref() == Some(pane) || issue.agent == pane)
    .map(|(key, _)| key.clone())
}

pub(super) fn field<'a>(line: &'a Value, key: &str) -> Option<&'a str> {
  line
    .pointer(&format!("/data/{key}"))
    .and_then(Value::as_str)
    .or_else(|| line.get(key).and_then(Value::as_str))
}

#[cfg(test)]
#[path = "tests/identity_test.rs"]
mod tests;
