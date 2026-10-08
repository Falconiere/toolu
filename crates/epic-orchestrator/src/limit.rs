//! Provider-limit lines, matching the epic watcher's expression.

use std::time::{Duration, UNIX_EPOCH};

use regex::RegexBuilder;
use serde_json::{Value, json};
use toolu_runtime::json::ordered::Ordered;
use toolu_state::time::iso_seconds;

use crate::disk::{read_value, write_value};
use crate::model::World;
use crate::paths::Paths;
use crate::schedule::note_event;
use crate::server::Engine;

/// The watcher expression, case insensitive, including its trailing space.
const PATTERN: &str = "usage limit|rate[- ]limit(ed)?\\b|hit your (usage )?limit|quota (exceeded|reached)|too many requests|\\b429\\b|limit will reset|try again (at|in) ";

/// The last non-empty line that matches the provider-limit pattern, at most 200 chars.
pub(crate) fn limit_line(tail: &str) -> Option<String> {
  let Ok(pattern) = RegexBuilder::new(PATTERN).case_insensitive(true).build() else {
    return None;
  };
  let line = tail
    .lines()
    .rev()
    .map(str::trim)
    .find(|line| !line.is_empty() && pattern.is_match(line))?;
  Some(truncate(line))
}

const COOL_MIN: u64 = 60;
const HOSTS: &[&str] = &[
  "claude",
  "claude-code",
  "codex",
  "cursor",
  "cursor-agent",
  "opencode",
];

/// A failed report whose note starts with `rate-limited`.
pub(crate) fn on_failed(
  engine: &mut Engine,
  phase: &str,
  note: &str,
  key: &str,
) -> Result<(), String> {
  if phase == "ready" || phase == "needs-human" || !rate_limited(note) {
    return Ok(());
  }
  let line = limit_line(note).unwrap_or_else(|| truncate(note));
  cool(engine, key, &line)
}

/// Scan pane text once per phase when the status is idle, done, or blocked.
pub(crate) fn on_status(
  engine: &mut Engine,
  key: &str,
  status: &str,
  tail: &str,
) -> Result<(), String> {
  if parked(engine, key) {
    return Ok(());
  }
  if status == "working" {
    if let Some(issue) = engine.world.issues.get_mut(key) {
      issue.blocked = false;
    }
    return Ok(());
  }
  if status == "blocked" {
    mark_blocked(engine, key);
  } else if let Some(issue) = engine.world.issues.get_mut(key) {
    issue.blocked = false;
  }
  if !matches!(status, "idle" | "done" | "blocked") {
    return Ok(());
  }
  let due = engine
    .world
    .issues
    .get(key)
    .is_some_and(|issue| issue.scanned_at != Some(issue.phase_at_ms));
  if !due {
    return Ok(());
  }
  if let Some(issue) = engine.world.issues.get_mut(key) {
    issue.scanned_at = Some(issue.phase_at_ms);
  }
  let Some(line) = limit_line(tail) else {
    return Ok(());
  };
  cool(engine, key, &line)
}

fn parked(engine: &Engine, key: &str) -> bool {
  engine
    .world
    .issues
    .get(key)
    .is_some_and(|issue| matches!(issue.phase.as_str(), "ready" | "needs-human"))
}

fn mark_blocked(engine: &mut Engine, key: &str) {
  let already = engine
    .world
    .issues
    .get(key)
    .is_some_and(|issue| issue.blocked);
  if already {
    return;
  }
  if let Some(issue) = engine.world.issues.get_mut(key) {
    issue.blocked = true;
  }
  note_event(&mut engine.world, key, "blocked");
}

fn rate_limited(note: &str) -> bool {
  note.trim().to_ascii_lowercase().starts_with("rate-limited")
}

fn cool(engine: &mut Engine, key: &str, line: &str) -> Result<(), String> {
  note_event(&mut engine.world, key, "host-limited");
  let kind = engine
    .world
    .issues
    .get(key)
    .map(|issue| issue.kind.clone())
    .unwrap_or_default();
  if !HOSTS.contains(&kind.as_str()) {
    return flush_limit(engine);
  }
  write_host(engine, key, &kind, line)?;
  write_cooldown(&engine.paths, &engine.world, &kind, line)?;
  flush_limit(engine)
}

fn flush_limit(engine: &mut Engine) -> Result<(), String> {
  engine.flush()
}

fn write_host(engine: &Engine, key: &str, kind: &str, line: &str) -> Result<(), String> {
  let Some(dir) = engine
    .world
    .issues
    .get(key)
    .map(|issue| issue.state_dir.clone())
  else {
    return Ok(());
  };
  let path = std::path::Path::new(&dir).join("hosts.json");
  let mut map = match read_value(&path)? {
    Value::Object(map) => map,
    Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) | Value::Array(_) => {
      serde_json::Map::new()
    }
  };
  let millis = engine.world.now_ms.saturating_add(COOL_MIN * 60 * 1000);
  let until = UNIX_EPOCH
    .checked_add(Duration::from_millis(millis))
    .map_or_else(String::new, iso_seconds);
  map.insert(kind.to_owned(), json!({"until": until, "reason": line}));
  write_value(&path, &Value::Object(map))
}

fn write_cooldown(paths: &Paths, world: &World, kind: &str, line: &str) -> Result<(), String> {
  let until = world.now_ms.saturating_add(COOL_MIN * 60 * 1000);
  let until = serde_json::Number::from(i64::try_from(until).unwrap_or(i64::MAX));
  toolu_engine::resources::store::update_resources(&paths.root, |state| {
    let mut cooldowns = state
      .get("cooldowns")
      .cloned()
      .unwrap_or_else(|| Ordered::Object(Vec::new()));
    cooldowns.set(
      kind,
      Ordered::Object(vec![
        ("until".to_owned(), Ordered::Number(until)),
        ("reason".to_owned(), Ordered::String(line.to_owned())),
      ]),
    );
    state.set("cooldowns", cooldowns);
    Ok(())
  })
}

fn truncate(line: &str) -> String {
  let mut out = String::new();
  for ch in line.chars().take(200) {
    out.push(ch);
  }
  out
}

#[cfg(test)]
#[path = "tests/limit_test.rs"]
mod tests;
