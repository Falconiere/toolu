//! Token use of a Muse Code session: the `goal_usage_attribution` records of
//! `<data>/sessions/<yyyy>/<mm>/<dd>/<id>/session.jsonl` and of its
//! `subagent/<child>/session.jsonl` logs (a child's use is never in the
//! parent's log). Only records Muse marks `reported` count, each `usage_id`
//! once.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use toolu_runtime::env::Env;

use crate::launch::muse_session::data_root;

const FIELDS: [&str; 4] = [
  "input_tokens",
  "output_tokens",
  "cached_tokens",
  "reasoning_tokens",
];

/// The directory named `name` under `dir`, at most `depth` levels down.
fn find_dir(dir: &Path, name: &str, depth: u32) -> Option<PathBuf> {
  let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
    .ok()?
    .filter_map(|entry| entry.ok().map(|entry| entry.path()))
    .filter(|path| path.is_dir())
    .collect();
  entries.sort();
  if let Some(found) = entries
    .iter()
    .find(|path| path.file_name().is_some_and(|file| file == name))
  {
    return Some(found.clone());
  }
  if depth == 0 {
    return None;
  }
  entries
    .iter()
    .find_map(|path| find_dir(path, name, depth - 1))
}

/// The session's own log and its subagents' logs.
fn logs(session: &Path) -> Vec<PathBuf> {
  let mut logs = vec![session.join("session.jsonl")];
  if let Ok(children) = std::fs::read_dir(session.join("subagent")) {
    let mut paths: Vec<PathBuf> = children
      .filter_map(|entry| entry.ok().map(|entry| entry.path().join("session.jsonl")))
      .collect();
    paths.sort();
    logs.extend(paths);
  }
  logs
}

/// Every reported quantity in `text`, keyed by its `usage_id`.
fn reported(text: &str, into: &mut BTreeMap<String, Value>) {
  for entry in text
    .lines()
    .filter_map(|line| serde_json::from_str::<Value>(line).ok())
  {
    if entry.pointer("/payload/event/kind").and_then(Value::as_str)
      != Some("goal_usage_attribution")
    {
      continue;
    }
    let Some(record) = entry.pointer("/payload/event/record") else {
      continue;
    };
    let id = record.get("usage_id").and_then(Value::as_str);
    let quantity = record.get("quantity");
    let counted = quantity
      .and_then(|quantity| quantity.get("reported"))
      .and_then(Value::as_bool)
      == Some(true);
    if let (Some(id), Some(quantity), true) = (id, quantity, counted) {
      into.insert(id.to_owned(), quantity.clone());
    }
  }
}

/// The token sums of Muse session `session`.
///
/// # Errors
/// No session directory, or a log in it cannot be read.
pub(super) fn muse(session: &str, env: &Env) -> Result<(&'static str, Value), &'static str> {
  let sessions = data_root(env).join("sessions");
  let dir = find_dir(&sessions, session, 3).ok_or("no session log for the session")?;
  let mut usage = BTreeMap::new();
  for (index, log) in logs(&dir).iter().enumerate() {
    match std::fs::read_to_string(log) {
      Ok(text) => reported(&text, &mut usage),
      // A subagent directory without a log yet holds no use.
      Err(err) if index > 0 && err.kind() == std::io::ErrorKind::NotFound => {}
      Err(_err) if index == 0 => return Err("the session log cannot be read"),
      Err(_err) => return Err("a subagent log cannot be read"),
    }
  }
  let sum = |field: &str| -> u64 {
    usage
      .values()
      .map(|quantity| quantity.get(field).and_then(Value::as_u64).unwrap_or(0))
      .fold(0, u64::saturating_add)
  };
  let tokens: serde_json::Map<String, Value> = FIELDS
    .iter()
    .map(|field| ((*field).to_owned(), json!(sum(field))))
    .collect();
  Ok(("muse session log", Value::Object(tokens)))
}

#[cfg(test)]
#[path = "tests/muse_tokens_test.rs"]
mod tests;
