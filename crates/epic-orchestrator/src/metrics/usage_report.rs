//! Read-only usage reports: attempt provenance, deduplicated native sessions and
//! normalized known subtotals. Missing counters never become invented zeros.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;

use serde_json::{Value, json};
use toolu_runtime::env::Env;

use super::usage::{self, Attempt};
use crate::launch::record::{Record, text};

#[derive(Default)]
struct Report {
  attempts: Vec<Value>,
  sessions: BTreeMap<(String, String), (String, Attempt)>,
  reasons: BTreeSet<String>,
}

/// Build a report without modifying issue records or native session logs.
///
/// # Errors
/// Records cannot be read, history is malformed, or counter arithmetic overflows.
pub(crate) fn report(state: &Path, env: &Env) -> Result<Value, String> {
  let dir = state.join("issues");
  let entries = std::fs::read_dir(&dir).map_err(|err| format!("{}: {err}", dir.display()))?;
  let mut paths = Vec::new();
  for entry in entries {
    let path = entry.map_err(|err| err.to_string())?.path();
    if path.extension().is_some_and(|ext| ext == "json") {
      paths.push(path);
    }
  }
  paths.sort();
  let mut report = Report::default();
  for path in paths {
    let Value::Object(mut record) = crate::disk::read_value(&path)? else {
      return Err(format!("invalid usage record {}", path.display()));
    };
    crate::launch::record::require_local_record(&record, &path)?;
    usage::enroll(&mut record, env)?;
    collect(&mut report, &record, env)?;
  }
  finish(report)
}

fn collect(report: &mut Report, record: &Record, env: &Env) -> Result<(), String> {
  let key = text(record, "key").ok_or("usage record has no issue key")?;
  let history = usage::load(record)?.ok_or("usage history was not enrolled")?;
  if history.legacy_gap {
    report
      .reasons
      .insert(format!("{key}: earlier launch history is incomplete"));
  }
  for mut attempt in history.attempts {
    usage::refresh(&mut attempt, key, env, false)?;
    report.attempts.push(attempt_row(key, &attempt));
    let Some(session) = &attempt.session else {
      report
        .reasons
        .insert(format!("{key} attempt {}: no captured session", attempt.id));
      continue;
    };
    let identity = (attempt.host.clone(), session.clone());
    if let Some((previous_key, previous)) = report.sessions.get(&identity) {
      if previous_key != key {
        report.reasons.insert(format!(
          "session {session} is attributed to multiple issues"
        ));
      }
      if attempt.row.get("tokens").is_none() && previous.row.get("tokens").is_some() {
        let reason = attempt.row.get("reason").cloned();
        attempt.row = previous.row.clone();
        attempt.observed_at.clone_from(&previous.observed_at);
        let row = attempt
          .row
          .as_object_mut()
          .ok_or("invalid usage history row")?;
        row.insert("status".to_owned(), json!("stale"));
        row.insert(
          "reason".to_owned(),
          reason.unwrap_or_else(|| json!("latest attempt has no usage")),
        );
        attempt.finalized = false;
      }
    }
    report.sessions.insert(identity, (key.to_owned(), attempt));
  }
  Ok(())
}

fn attempt_row(key: &str, attempt: &Attempt) -> Value {
  json!({
    "key": key, "id": attempt.id, "host": attempt.host, "session": attempt.session,
    "worktree": attempt.worktree, "started_at": attempt.started_at,
    "observed_at": attempt.observed_at, "final": attempt.finalized, "row": attempt.row,
  })
}

fn add(left: u64, right: u64) -> Result<u64, String> {
  left
    .checked_add(right)
    .ok_or_else(|| "usage counters overflow".to_owned())
}

fn normalized(attempt: &Attempt) -> Result<Option<(u64, u64)>, String> {
  let Some(tokens) = attempt.row.get("tokens") else {
    return Ok(None);
  };
  let field = |name: &str| tokens.get(name).and_then(Value::as_u64);
  let names = match attempt.host.as_str() {
    "grok" => ("inputTokens", "outputTokens"),
    "claude" | "codex" | "muse" => ("input_tokens", "output_tokens"),
    _ => return Ok(None),
  };
  let (Some(mut input), Some(output)) = (field(names.0), field(names.1)) else {
    return Ok(None);
  };
  if attempt.host == "claude" {
    let (Some(read), Some(created)) = (
      field("cache_read_input_tokens"),
      field("cache_creation_input_tokens"),
    ) else {
      return Ok(None);
    };
    input = add(add(input, read)?, created)?;
  }
  Ok(Some((input, output)))
}

fn finish(mut report: Report) -> Result<Value, String> {
  let mut sessions = Vec::new();
  let (mut input, mut output) = (0, 0);
  for (key, attempt) in report.sessions.values() {
    if !attempt.finalized || attempt.row.get("status").is_some() {
      let reason = attempt
        .row
        .get("reason")
        .and_then(Value::as_str)
        .unwrap_or("provisional snapshot; source has not been finalized");
      report
        .reasons
        .insert(format!("{key} attempt {}: {reason}", attempt.id));
    }
    if let Some((incoming, outgoing)) = normalized(attempt)? {
      input = add(input, incoming)?;
      output = add(output, outgoing)?;
      sessions.push(attempt_row(key, attempt));
    } else {
      report.reasons.insert(format!(
        "{key} attempt {}: normalized usage unavailable",
        attempt.id
      ));
    }
  }
  let known = !sessions.is_empty();
  let coverage = match (known, report.reasons.is_empty()) {
    (false, _) => "unavailable",
    (true, false) => "partial",
    (true, true) => "complete",
  };
  let total = add(input, output)?;
  Ok(json!({
    "schema": 1, "coverage": coverage, "reasons": report.reasons,
    "attempts": report.attempts,
    "totals": {"sessions": sessions.len(), "input_tokens": known.then_some(input),
      "output_tokens": known.then_some(output), "total_tokens": known.then_some(total)},
    "sessions": sessions,
  }))
}

fn cell(value: Option<&Value>) -> String {
  match value {
    Some(Value::String(text)) => text.replace(['\n', '\r', '|'], " "),
    None | Some(Value::Null) => "—".to_owned(),
    Some(value) => value.to_string(),
  }
}

/// Append private usage detail to a human summary; callers omit it publicly.
pub(super) fn markdown(usage: &Value, out: &mut String) {
  let _written = writeln!(
    out,
    "\n### Agent usage\n\nCoverage: **{}**. Known token subtotal: {}.",
    cell(usage.get("coverage")),
    cell(usage.pointer("/totals/total_tokens"))
  );
  out.push_str("\n| Issue | Attempt | Host | Session | Final | Observed | Native tokens / reason |\n|---|---|---|---|---|---|---|\n");
  if let Some(attempts) = usage.get("attempts").and_then(Value::as_array) {
    for attempt in attempts {
      let mut cells: Vec<String> = ["key", "id", "host", "session", "final", "observed_at"]
        .iter()
        .map(|name| cell(attempt.get(name)))
        .collect();
      cells.push(cell(
        attempt
          .pointer("/row/tokens")
          .or_else(|| attempt.pointer("/row/reason")),
      ));
      let _written = writeln!(out, "| {} |", cells.join(" | "));
    }
  }
  if let Some(reasons) = usage.get("reasons").and_then(Value::as_array) {
    for reason in reasons {
      let _written = writeln!(out, "\n- {}", cell(Some(reason)));
    }
  }
}

#[cfg(test)]
#[path = "tests/usage_report_test.rs"]
mod tests;
