//! Socket operations protocol 2 adds: `release {key}` clears a coupling
//! hold so the issue launches next, and `finish {key, step}` brackets a
//! `toolu epic finish` run in another process, so the engine neither
//! relaunches the issue it tears down nor keeps a stale stage (#619).

use std::sync::mpsc::Sender;

use serde_json::{Value, json};

use crate::dispatch::pump_after;
use crate::journal::Record;
use crate::model::{Action, Step, World};
use crate::server::Engine;

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
