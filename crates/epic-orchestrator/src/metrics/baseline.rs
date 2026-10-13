//! The TypeScript-era baseline (#451), derived from the watcher's state
//! directories: `<state>/<epic>/issues/<key>.json` (launch, finish, stage,
//! launches) and `<state>/<epic>/status/<key>.json` (the phase history).
//! Outlier epics are measured apart so they do not dominate the totals.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};
use toolu_state::time::{epoch_millis, parse_iso};

use crate::metrics::stats::{self, decimal, pct, share};

/// What `toolu epic baseline` was asked for.
#[derive(Debug, Clone)]
pub(crate) struct Input {
  /// The state root holding one directory per epic.
  pub state: PathBuf,
  /// The cutoff, as given.
  pub until: String,
  /// Epics reported apart.
  pub outliers: Vec<String>,
}

#[derive(Debug, Default)]
struct Group {
  epics: BTreeSet<String>,
  lead: Vec<u64>,
  ready: Vec<u64>,
  phases: BTreeMap<String, u64>,
  rebases: u64,
  relaunched: u64,
}

#[derive(Debug, Default)]
struct Totals {
  records: u64,
  merged: u64,
  main: Group,
  outliers: BTreeMap<String, Group>,
}

/// The baseline document.
///
/// # Errors
/// The cutoff is not a timestamp, or a state file cannot be read or parsed.
pub(crate) fn derive(input: &Input) -> Result<Value, String> {
  let until = parse_iso(&input.until)
    .map(epoch_millis)
    .ok_or_else(|| format!("bad --until {:?}: want an ISO-8601 UTC time", input.until))?;
  let mut totals = Totals::default();
  for epic in sorted_dirs(&input.state)? {
    let issues = epic.join("issues");
    if !issues.is_dir() {
      continue;
    }
    let name = file_name(&epic);
    for path in sorted_files(&issues)? {
      let group = if input.outliers.contains(&name) {
        totals.outliers.entry(name.clone()).or_default()
      } else {
        &mut totals.main
      };
      let (counted, merged) = issue(&epic, &path, until, &name, group)?;
      totals.records = totals.records.saturating_add(u64::from(counted));
      totals.merged = totals.merged.saturating_add(u64::from(merged));
    }
  }
  let outliers: Map<String, Value> = totals
    .outliers
    .iter()
    .map(|(name, group)| (name.clone(), summary(group)))
    .collect();
  Ok(json!({
    "schema": 1,
    "source": {
      "until": input.until, "outliers": input.outliers,
      "records": totals.records, "merged": totals.merged,
    },
    "main": summary(&totals.main),
    "outliers": outliers,
    "reported": reported(),
  }))
}

/// One issue record: whether it counts by `until`, and whether it merged.
fn issue(
  epic: &Path,
  path: &Path,
  until: u64,
  name: &str,
  group: &mut Group,
) -> Result<(bool, bool), String> {
  let record = read(path)?;
  let Some(launched) = time(&record, "launched_at", path)? else {
    return Ok((false, false));
  };
  if launched > until {
    return Ok((false, false));
  }
  let finished = time(&record, "finished_at", path)?.filter(|at| *at <= until);
  let merged = record.get("stage").and_then(Value::as_str) == Some("merged");
  let Some(finished) = finished.filter(|_| merged) else {
    return Ok((true, false));
  };
  group.epics.insert(name.to_owned());
  group.lead.push(finished.saturating_sub(launched));
  if record.get("launches").and_then(Value::as_u64).unwrap_or(1) > 1 {
    group.relaunched = group.relaunched.saturating_add(1);
  }
  let status = epic
    .join("status")
    .join(path.file_name().unwrap_or_default());
  if status.is_file() {
    history(&read(&status)?, finished, &status, group)?;
  }
  Ok((true, true))
}

/// Phase times, rebase rounds and first `ready` from a status history.
fn history(status: &Value, finished: u64, path: &Path, group: &mut Group) -> Result<(), String> {
  let mut entries = Vec::new();
  for entry in status
    .get("history")
    .and_then(Value::as_array)
    .into_iter()
    .flatten()
  {
    let phase = entry.get("phase").and_then(Value::as_str).unwrap_or("");
    if let Some(at) = time(entry, "at", path)?.filter(|at| *at <= finished) {
      entries.push((phase, at));
    }
  }
  if let Some((_, at)) = entries.iter().find(|(phase, _)| *phase == "ready") {
    group.ready.push(finished.saturating_sub(*at));
  }
  let ends = entries.iter().skip(1).map(|(_, at)| *at).chain([finished]);
  for ((phase, at), end) in entries.iter().zip(ends) {
    let spent = group.phases.entry((*phase).to_owned()).or_default();
    *spent = spent.saturating_add(end.saturating_sub(*at));
    if *phase == "rebasing" {
      group.rebases = group.rebases.saturating_add(1);
    }
  }
  Ok(())
}

fn summary(group: &Group) -> Value {
  let total = stats::total(&group.lead);
  let phase_total = stats::total(group.phases.values());
  let phases: Map<String, Value> = group
    .phases
    .iter()
    .map(|(phase, ms)| {
      (
        phase.clone(),
        json!({"ms": ms, "share": share(*ms, phase_total)}),
      )
    })
    .collect();
  json!({
    "epics": group.epics.len(),
    "merged_issues": group.lead.len(),
    "agent_hours": decimal(total.saturating_mul(10).saturating_add(1_800_000) / 3_600_000, 10),
    "lead_time_ms": pct(&group.lead),
    "ready_to_cleanup_ms": pct(&group.ready),
    "phases": phases,
    "rebase_rounds": group.rebases,
    "relaunched_issues": group.relaunched,
  })
}

/// The hand-computed figures #451 recorded on 2026-10-04, kept verbatim.
fn reported() -> Value {
  json!({
    "date": "2026-10-04",
    "lead_time_min": {"p50": 219, "p90": 912},
    "first_ready_to_cleanup_min": {"p50": 1, "p90": 69},
    "supervisor_rss_mb_per_epic": 31,
    "note": "hand-computed for #451 and the engine brainstorm; method not recorded",
  })
}

fn time(value: &Value, field: &str, path: &Path) -> Result<Option<u64>, String> {
  match value.get(field) {
    None | Some(Value::Null) => Ok(None),
    Some(Value::String(text)) => parse_iso(text)
      .map(|at| Some(epoch_millis(at)))
      .ok_or_else(|| format!("{}: bad {field} {text:?}", path.display())),
    Some(other) => Err(format!("{}: bad {field} {other}", path.display())),
  }
}

fn read(path: &Path) -> Result<Value, String> {
  let text = std::fs::read_to_string(path).map_err(|err| format!("{}: {err}", path.display()))?;
  serde_json::from_str(&text).map_err(|err| format!("{}: {err}", path.display()))
}

fn sorted_dirs(root: &Path) -> Result<Vec<PathBuf>, String> {
  Ok(
    listing(root)?
      .into_iter()
      .filter(|path| path.is_dir())
      .collect(),
  )
}

fn sorted_files(dir: &Path) -> Result<Vec<PathBuf>, String> {
  Ok(
    listing(dir)?
      .into_iter()
      .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
      .collect(),
  )
}

fn listing(dir: &Path) -> Result<Vec<PathBuf>, String> {
  let entries = std::fs::read_dir(dir).map_err(|err| format!("{}: {err}", dir.display()))?;
  let mut paths = Vec::new();
  for entry in entries {
    paths.push(
      entry
        .map_err(|err| format!("{}: {err}", dir.display()))?
        .path(),
    );
  }
  paths.sort();
  Ok(paths)
}

fn file_name(path: &Path) -> String {
  path
    .file_name()
    .map(|name| name.to_string_lossy().into_owned())
    .unwrap_or_default()
}

#[cfg(test)]
#[path = "tests/baseline_test.rs"]
mod tests;
