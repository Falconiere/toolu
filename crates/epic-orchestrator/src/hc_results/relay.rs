//! Import a verified relay result and start local review.

use super::{field, mirror, review};
use crate::launch::record::{Record, text, write_record};
use serde_json::{Value, json};
use std::path::Path;
use toolu_mux::hc::remote;
use toolu_runtime::env::Env;

/// Import a completed task or report a held or failed one.
pub(super) fn result(path: &Path, record: &mut Record, env: &Env) -> Result<Option<Value>, String> {
  let request: toolu_mux::relay::store::Submission = serde_json::from_value(
    record
      .get("relay_request")
      .cloned()
      .ok_or("missing relay request")?,
  )
  .map_err(|e| e.to_string())?;
  let task = toolu_mux::relay::client::get(env, &request.key)?;
  if task.request != request {
    return Err("relay request differs from saved dispatch".into());
  }
  if task.phase == "held" {
    return Err(format!(
      "relay task held: {}",
      task.note.as_deref().unwrap_or("inspect relay status")
    ));
  }
  if !matches!(task.phase.as_str(), "done" | "failed" | "cancelled") {
    return Ok(None);
  }
  let commit = if task.phase == "done" {
    task
      .result
      .as_deref()
      .ok_or("missing relay result commit")?
  } else {
    &request.base
  };
  let phase = if task.phase == "done" {
    "ready"
  } else {
    "failed"
  };
  let report = json!({"run": field(record, "hc_run")?, "expected_base": request.base,
    "commit": commit, "phase": phase, "note": task.note.as_deref().unwrap_or_default()});
  if record.get("remote_report_seen") == Some(&report) {
    return Ok(None);
  }
  if phase == "ready" {
    accept(path, record, env, &task, commit)?;
  }
  Ok(Some(report))
}

fn accept(
  path: &Path,
  record: &mut Record,
  env: &Env,
  task: &toolu_mux::relay::store::Task,
  commit: &str,
) -> Result<(), String> {
  if record.get("remote_result_imported") == Some(&Value::Bool(true)) {
    if text(record, "remote_result_commit") != Some(commit) {
      return Err("relay result changed after import".into());
    }
  } else {
    let attempt = task.attempt.as_deref().ok_or("missing relay attempt")?;
    let bytes = toolu_mux::relay::client::output(env, &task.request.key)?;
    remote::import_recover(env, field(record, "checkout")?, attempt, commit, &bytes)
      .map_err(|e| e.message)?;
    mirror::advance_mirror(record, env, commit)?;
    record.insert("remote_result_commit".into(), json!(commit));
    record.insert("remote_result_imported".into(), json!(true));
    record.insert(
      "terminal_address".into(),
      json!(
        task
          .terminal
          .clone()
          .unwrap_or_else(|| format!("relay:{}", task.request.key))
      ),
    );
    write_record(path, record)?;
  }
  review::handoff(path, record, env)
}

#[cfg(test)]
#[path = "tests/relay_test.rs"]
mod tests;
