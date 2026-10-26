//! Extra socket operations: `release {key}` clears a coupling hold so the
//! issue launches next, `finish {key, step}` brackets a `toolu epic finish`
//! run in another process (#619), and `notify` journals a worker notice
//! with an optional realtime relay to one sibling issue.

use std::sync::mpsc::Sender;

use serde_json::{Value, json};

use crate::dispatch::{pump_after, remember_spool, spool_seen};
use crate::journal::Record;
use crate::model::{Action, Step, World};
use crate::server::Engine;

/// Journal a worker notice, raise its attention, and relay an addressed
/// notice to its sibling at once.
pub(crate) fn reply_notify(
  engine: &mut Engine,
  request: &Value,
  reply: &Sender<Value>,
) -> Result<bool, String> {
  if let Some(token) = request.get("token").and_then(Value::as_str)
    && !token.is_empty()
    && spool_seen(engine, token)?
  {
    let _sent = reply.send(json!({"ok": true}));
    return Ok(false);
  }
  if let Err(err) = apply_notify(engine, request) {
    let _sent = reply.send(json!({"error": err}));
    return Ok(false);
  }
  remember_spool(engine, request)?;
  engine.flush()?;
  let _sent = reply.send(json!({"ok": true}));
  pump_after(engine)
}

/// Validate and record one worker notice: its journal row, attention item,
/// and addressed relay. A relay goes only to one running sibling the sender
/// names; anything else stays orchestrator-only.
pub(crate) fn apply_notify(engine: &mut Engine, request: &Value) -> Result<(), String> {
  let key = request.get("key").and_then(Value::as_str).unwrap_or("");
  let kind = request.get("kind").and_then(Value::as_str).unwrap_or("");
  let note = request.get("note").and_then(Value::as_str).unwrap_or("");
  let to = request.get("to").and_then(Value::as_str).unwrap_or("");
  let token = request.get("token").and_then(Value::as_str).unwrap_or("");
  if key.is_empty() {
    return Err("notify needs a key".to_owned());
  }
  if !crate::verbs_notify::NOTIFY_KINDS.contains(&kind) {
    return Err(format!("unknown kind {kind}"));
  }
  if !to.is_empty() && to != key && !engine.world.issues.contains_key(to) {
    return Err(format!("unknown recipient {to}"));
  }
  engine
    .world
    .outbox
    .push_back(Step::Journal(Record::new("notify", kind, key, token, note)));
  crate::attention::reraise(&mut engine.world, kind, key, note);
  if !to.is_empty() && to != key && crate::attention::has_live_agent(&engine.world, to) {
    relay(engine, key, kind, note, to);
  }
  Ok(())
}

/// The quiet window between two relays to one recipient.
const RELAY_COOLDOWN_MS: u64 = 60_000;

/// Relay one worker's notice to a running sibling at once, journaling the
/// delivery like a babysit prompt. Delivery itself stays best-effort: the
/// caller already got its journal row and attention item, so a deferred
/// herdr send keeps the `ok`. A repeat inside the cooldown window stays
/// orchestrator-only.
fn relay(engine: &mut Engine, from: &str, kind: &str, note: &str, to: &str) {
  let now = engine.world.now_ms;
  if engine
    .world
    .relayed_at_ms
    .get(to)
    .is_some_and(|at| now < at.saturating_add(RELAY_COOLDOWN_MS))
  {
    return;
  }
  let text = relay_text(from, kind, note);
  engine
    .world
    .outbox
    .push_back(Step::Journal(Record::new("prompt", "relay", to, "", &text)));
  engine.world.outbox.push_back(Step::Prompt {
    key: to.to_owned(),
    text: text.clone(),
    // No fallback: the notice already raised its attention item above. The
    // direction stays visible in status for workers that poll instead.
    fallback: None,
  });
  engine.world.relayed_at_ms.insert(to.to_owned(), now);
  if let Some(issue) = engine.world.issues.get_mut(to) {
    issue.direction = Some(crate::model::Direction {
      text,
      by: format!("relay:{from}"),
      at_ms: now,
    });
  }
}

/// The engine-signed prompt relaying one worker's notice to a sibling.
fn relay_text(from: &str, kind: &str, note: &str) -> String {
  let head = if note.is_empty() {
    format!("relay from {from} ({kind})")
  } else {
    format!("relay from {from} ({kind}): {note}")
  };
  format!("{head} Stay idle unless this needs you.")
}

/// Release `request.key`'s hold, or answer why not.
///
/// # Errors
/// A journal or record write fails.
pub(crate) fn reply_release(
  engine: &mut Engine,
  request: &Value,
  reply: &Sender<Value>,
) -> Result<bool, String> {
  let key = request.get("key").and_then(Value::as_str).unwrap_or("");
  if let Err(err) = crate::holds::release(&mut engine.world, key) {
    let _sent = reply.send(json!({"error": err}));
    return Ok(false);
  }
  engine.flush()?;
  let _sent = reply.send(json!({"ok": true, "key": key, "released": true}));
  pump_after(engine)
}

/// `begin` marks the issue `cleaning`, so its teardown's herdr events arm
/// no relaunch; `end` adopts the stage the finish wrote to the record.
///
/// # Errors
/// A journal write fails.
pub(crate) fn reply_finish(
  engine: &mut Engine,
  request: &Value,
  reply: &Sender<Value>,
) -> Result<bool, String> {
  let key = request.get("key").and_then(Value::as_str).unwrap_or("");
  let answer = match request.get("step").and_then(Value::as_str) {
    Some("begin") => begin(&mut engine.world, key),
    Some("end") => end(&mut engine.world, key, request.get("stage")),
    _ => Err("finish needs step begin or end".to_owned()),
  };
  let answer = match answer {
    Ok(answer) => answer,
    Err(err) => {
      let _sent = reply.send(json!({"error": err}));
      return Ok(false);
    }
  };
  engine.flush()?;
  let _sent = reply.send(answer);
  pump_after(engine)
}

fn untracked(key: &str) -> Value {
  json!({"ok": true, "key": key, "tracked": false})
}

fn journal(world: &mut World, key: &str, note: &str) {
  let line = Record::new("action", "finish", key, "", note);
  world.outbox.push_back(Step::Journal(line));
}

fn begin(world: &mut World, key: &str) -> Result<Value, String> {
  let Some(issue) = world.issues.get_mut(key) else {
    return Ok(untracked(key));
  };
  let action = issue.pending.as_ref().map(|pending| pending.action);
  if matches!(action, Some(Action::Merge | Action::Cleanup)) {
    return Err(format!(
      "the engine is merging or cleaning up {key}; wait for it"
    ));
  }
  let previous = std::mem::replace(&mut issue.stage, "cleaning".to_owned());
  issue.relaunch = None;
  if let Some(pending) = issue.pending.as_mut() {
    pending.follow = None;
  }
  journal(world, key, "begin");
  Ok(json!({"ok": true, "key": key, "tracked": true, "stage": previous}))
}

fn end(world: &mut World, key: &str, previous: Option<&Value>) -> Result<Value, String> {
  let Some(issue) = world.issues.get_mut(key) else {
    return Ok(untracked(key));
  };
  let path = crate::snapshot::issue_path(&issue.state_dir, key);
  let recorded = crate::disk::read_value(&path)?;
  let stage = recorded
    .get("stage")
    .and_then(Value::as_str)
    .filter(|stage| !stage.is_empty())
    .or_else(|| previous.and_then(Value::as_str))
    .unwrap_or("")
    .to_owned();
  stage.clone_into(&mut issue.stage);
  if matches!(stage.as_str(), "merged" | "abandoned") {
    crate::attention::settle(world, key, None, "finish");
  }
  journal(world, key, &stage);
  Ok(json!({"ok": true, "key": key, "stage": stage}))
}

/// Give back the record's live stage to each issue of `epic` that a finish
/// left `cleaning` with no action pending, once no finish holds its launch
/// lock: the CLI died between `begin` and `end` after refusing before it
/// wrote anything, so its agent still runs (#619).
pub(crate) fn heal_cleaning(world: &mut World, epic: &str) {
  let stale: Vec<(String, String)> = world
    .issues
    .values()
    .filter(|issue| issue.epic == epic && issue.stage == "cleaning" && issue.pending.is_none())
    .filter_map(|issue| {
      let path = crate::snapshot::issue_path(&issue.state_dir, &issue.key);
      let record = crate::disk::read_value(&path).ok()?;
      let stage = record.get("stage").and_then(Value::as_str)?;
      let live = matches!(stage, "starting" | "uncertain" | "running");
      (live && !finish_holds(&issue.state_dir, &issue.key))
        .then(|| (issue.key.clone(), stage.to_owned()))
    })
    .collect();
  for (key, stage) in stale {
    if let Some(issue) = world.issues.get_mut(&key) {
      stage.clone_into(&mut issue.stage);
    }
    journal(world, &key, &stage);
  }
}

/// Whether a finish (or a launch) holds `key`'s launch lock; one that cannot
/// be probed counts as held, so nothing is healed on a guess.
fn finish_holds(state_dir: &str, key: &str) -> bool {
  let path = std::path::Path::new(state_dir)
    .join("launch-locks")
    .join(key);
  match toolu_engine::resources::lock::acquire_lock(&path, std::time::Duration::ZERO) {
    Ok(Some(lock)) => {
      lock.release();
      false
    }
    Ok(None) | Err(_) => true,
  }
}

#[cfg(test)]
#[path = "tests/dispatch_ops_test.rs"]
mod tests;
