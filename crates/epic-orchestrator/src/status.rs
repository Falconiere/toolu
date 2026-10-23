//! The status document `toolu epic status --json` prints.

use serde_json::{Value, json};

use crate::attention;
use crate::model::{Attention, Issue, World};

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
      "effectsHeld": crate::github_budget::effects_held(world),
    },
    "issues": issues.iter().map(|issue| json!({
      "key": issue.key,
      "epic": issue.epic,
      "phase": issue.phase,
      "stage": issue.stage,
      "github": pr_times(world, issue),
    })).collect::<Vec<_>>(),
    "attention": world
      .attention
      .iter()
      .filter(|item| wanted_epic(epic, &item.epic))
      .map(|item| attention::document(item, world.now_ms))
      .collect::<Vec<_>>(),
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
  wanted_epic(epic, &issue.epic)
}

fn wanted_epic(epic: Option<&str>, key: &str) -> bool {
  match epic {
    None => true,
    Some(epic) => key == epic || key.ends_with(&format!("-{epic}")),
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

/// The `wait` reply for `scope` and the token of the item it hands out: the
/// oldest judgment the scope may take, else the timeout body and no token.
pub(crate) fn wait_body(world: &mut World, scope: Option<&str>) -> (Value, String) {
  match attention::take(world, scope) {
    Some(item) => (judgment(&item, world.now_ms), item.token),
    None => (json!({"state": "waiting"}), String::new()),
  }
}

/// The `wait` document: the public fields, plus a blocked agent's screen text.
fn judgment(item: &Attention, now_ms: u64) -> Value {
  let mut value = attention::document(item, now_ms);
  if let (Some(pending), Some(map)) = (&item.pending, value.as_object_mut()) {
    let _previous = map.insert("pending".to_owned(), json!(pending));
  }
  value
}

#[cfg(test)]
#[path = "tests/status_test.rs"]
mod tests;
