//! The status document `toolu epic status --json` prints.

use serde_json::{Value, json};

use crate::model::{Attention, Issue, World};
use crate::schedule::take_judgment;

/// One status document. `epic` keeps issues for that epic key when set.
pub(crate) fn document(world: &World, running: bool, epic: Option<&str>) -> Value {
  let issues: Vec<&Issue> = world
    .issues
    .values()
    .filter(|issue| wanted(epic, issue))
    .collect();
  json!({
    "engine": if running { "running" } else { "down" },
    "paused": world.paused_all || !world.paused.is_empty(),
    "epics": epic_keys(&issues),
    "github": {
      "rest": {"rate": world.rest_rate, "points": world.rest_points},
      "graphql": {
        "remaining": world.graphql_remaining,
        "resetAt": world.graphql_reset_at,
        "points": world.graphql_points,
      },
      "holdUntil": world.github_hold_until_ms,
      "effectsHeld": crate::github_budget::low(world),
    },
    "issues": issues.iter().map(|issue| json!({
      "key": issue.key,
      "epic": issue.epic,
      "phase": issue.phase,
      "stage": issue.stage,
      "github": pr_times(world, issue),
    })).collect::<Vec<_>>(),
    "attention": world.attention.iter().filter(|item| !item.delivered).map(|item| json!({
      "kind": item.kind,
      "key": item.key,
      "epic": item.epic,
      "note": item.note,
      "seq": item.seq,
    })).collect::<Vec<_>>(),
  })
}

fn pr_times(world: &World, issue: &Issue) -> Value {
  let watch = world
    .watches
    .values()
    .find(|watch| matches!(&watch.kind, crate::watch::Kind::Pr { key, .. } if key == &issue.key));
  match watch {
    Some(watch) => json!({"lastCheckAt": watch.last_at_ms, "nextCheckAt": watch.next_at_ms}),
    None => Value::Null,
  }
}

fn wanted(epic: Option<&str>, issue: &Issue) -> bool {
  match epic {
    None => true,
    Some(epic) => issue.epic == epic || issue.epic.ends_with(&format!("-{epic}")),
  }
}

fn epic_keys(issues: &[&Issue]) -> Vec<Value> {
  let mut keys = Vec::new();
  for issue in issues {
    let key = json!({"key": issue.epic});
    if !keys.contains(&key) {
      keys.push(key);
    }
  }
  keys
}

/// `wait` when a judgment is already queued, otherwise the timeout body.
pub(crate) fn wait_body(world: &mut World, max_seconds: u64) -> Value {
  match take_judgment(world) {
    Some(item) => attention_value(&item),
    None => waiting(max_seconds),
  }
}

fn attention_value(item: &Attention) -> Value {
  json!({
    "kind": item.kind,
    "key": item.key,
    "epic": item.epic,
    "note": item.note,
    "seq": item.seq,
  })
}

fn waiting(_max_seconds: u64) -> Value {
  json!({"state": "waiting"})
}

#[cfg(test)]
#[path = "tests/status_test.rs"]
mod tests;
