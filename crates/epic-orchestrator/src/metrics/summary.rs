//! The end-of-epic report (#451): `toolu epic summary <epic>` and the
//! metrics section of the epic's closing comment. Each measure sits beside
//! the TypeScript baseline, then the budget table, then token use per worker
//! (the CLI only; a public comment carries no token rows).

use std::fmt::Write as _;
use std::path::Path;

use serde_json::{Value, json};
use toolu_runtime::env::Env;

use crate::metrics::text::{baseline, compared};
use crate::metrics::tokens::{self, agents};
use crate::metrics::{Query, budget_rows, document};
use crate::paths::Paths;

/// The summary document of `epic`: rows, budgets and, with `env`, token rows.
///
/// # Errors
/// The journal cannot be read, or no record names `epic`.
pub(crate) fn report(
  paths: &Paths,
  epic: &str,
  now_ms: u64,
  env: Option<&Env>,
) -> Result<Value, String> {
  let query = Query {
    epic: Some(epic.to_owned()),
    since_ms: None,
    now_ms,
  };
  let doc = document(&paths.journal_dir(), &query)?;
  if doc.get("issues").and_then(Value::as_u64).unwrap_or(0) == 0 {
    return Err(format!(
      "unknown epic {epic}: the journal holds no launch for it"
    ));
  }
  let rows: Vec<Value> = compared(&doc, &baseline())
    .into_iter()
    .map(|[measure, run, base]| json!({"measure": measure, "epic": run, "baseline": base}))
    .collect();
  let token_rows: Vec<Value> = match (env, state_dir(paths, epic)) {
    (Some(env), Some(dir)) => agents(&dir)
      .unwrap_or_default()
      .iter()
      .map(|agent| tokens::row(agent, env))
      .collect(),
    _ => Vec::new(),
  };
  Ok(json!({
    "schema": 1, "epic": epic, "rows": rows, "budgets": budget_rows(&doc), "tokens": token_rows,
  }))
}

/// The summary as Markdown; the token table only when the report has rows.
pub(crate) fn markdown(report: &Value) -> String {
  let list = |key: &str| {
    report
      .get(key)
      .and_then(Value::as_array)
      .cloned()
      .unwrap_or_default()
  };
  let text = |row: &Value, key: &str| match row.get(key) {
    Some(Value::String(text)) => text.clone(),
    Some(Value::Null) | None => "—".to_owned(),
    Some(other) => other.to_string(),
  };
  let mut out = String::from(
    "## Epic metrics\n\n| Measure | This epic | TypeScript baseline |\n|---|---|---|\n",
  );
  for row in list("rows") {
    let line = [
      text(&row, "measure"),
      text(&row, "epic"),
      text(&row, "baseline"),
    ]
    .join(" | ");
    let _written = writeln!(out, "| {line} |");
  }
  out.push_str("\n| Budget | Status | Value | Limit |\n|---|---|---|---|\n");
  for row in list("budgets") {
    let line = ["id", "status", "value", "budget"]
      .map(|key| text(&row, key))
      .join(" | ");
    let _written = writeln!(out, "| {line} |");
  }
  if !list("tokens").is_empty() {
    out.push_str("\n| Issue | Host | Tokens | Source |\n|---|---|---|---|\n");
    for row in list("tokens") {
      let tokens = row
        .get("tokens")
        .map_or_else(|| text(&row, "reason"), Value::to_string);
      let source = row
        .get("source")
        .map_or("unavailable".to_owned(), |_| text(&row, "source"));
      let line = [text(&row, "key"), text(&row, "host"), tokens, source].join(" | ");
      let _written = writeln!(out, "| {line} |");
    }
  }
  out
}

/// The closing comment's metrics section for the epic at `state`, or `None`
/// when the epic is not registered or the engine never ran it.
pub(crate) fn closing(paths: &Paths, state: &Path, now_ms: u64) -> Option<String> {
  let registered = crate::status_epics::registered(paths).ok()?;
  let (epic, _) = registered.into_iter().find(|(_, dir)| dir == state)?;
  let report = report(paths, &epic, now_ms, None).ok()?;
  Some(markdown(&report))
}

fn state_dir(paths: &Paths, epic: &str) -> Option<std::path::PathBuf> {
  crate::status_epics::registered(paths)
    .ok()?
    .into_iter()
    .find(|(key, _)| key == epic)
    .map(|(_, dir)| dir)
}

#[cfg(test)]
#[path = "tests/summary_test.rs"]
mod tests;
