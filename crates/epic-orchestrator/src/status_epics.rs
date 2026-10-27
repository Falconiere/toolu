//! Safe summaries of registered epic graphs and their live issues.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};
use toolu_runtime::config::secrets::{self, Secrets};

use crate::disk::read_value;
use crate::model::{Action, Issue, World};
use crate::paths::Paths;

const TEXT_LIMIT: usize = 160;

/// Registered epic keys and their state directories.
///
/// # Errors
/// The registry cannot be read.
pub(crate) fn registered(paths: &Paths) -> Result<Vec<(String, PathBuf)>, String> {
  let value = read_value(&paths.registry())?;
  Ok(
    value
      .get("epics")
      .and_then(Value::as_array)
      .into_iter()
      .flatten()
      .filter_map(|row| {
        let key = row.get("key")?.as_str()?.to_owned();
        let dir = PathBuf::from(row.get("state_dir")?.as_str()?);
        (!key.is_empty()).then_some((key, dir))
      })
      .collect(),
  )
}

/// All registered epic summaries, including an empty epic.
///
/// # Errors
/// The registry cannot be read.
pub(crate) fn list(world: &World, paths: &Paths, secrets: &Secrets) -> Result<Vec<Value>, String> {
  Ok(
    registered(paths)?
      .iter()
      .map(|(key, _)| summary(world, key, secrets))
      .collect(),
  )
}

fn epic_issues<'a>(world: &'a World, key: &str) -> Vec<&'a Issue> {
  world
    .issues
    .values()
    .filter(|issue| issue.epic == key)
    .collect()
}

fn summary(world: &World, key: &str, secrets: &Secrets) -> Value {
  let issues = epic_issues(world, key);
  let mut counts = BTreeMap::<String, u64>::new();
  for issue in &issues {
    *counts.entry(issue.stage.clone()).or_default() += 1;
  }
  let attention = world
    .attention
    .iter()
    .filter(|item| item.epic == key)
    .count();
  let mut value = json!({
    "key": key,
    "paused": world.paused_all || world.paused.contains(key),
    "counts": counts,
    "attention_count": attention,
  });
  if let Some(map) = value.as_object_mut() {
    map.extend(header(world, key, &issues, secrets));
  }
  value
}

/// The board header of `key`: title, URL, waves and the live and queued counts.
fn header(world: &World, key: &str, issues: &[&Issue], secrets: &Secrets) -> Map<String, Value> {
  let epic = world.epics.get(key);
  let graph = epic.and_then(|epic| epic.graph.as_ref());
  let done = done_keys(
    issues,
    graph.map_or(&[][..], |graph| graph.issues.as_slice()),
  );
  let waves = epic.map(|epic| &epic.waves);
  let current = waves
    .into_iter()
    .flatten()
    .filter(|(key, _)| !done.contains(key.as_str()))
    .map(|(_, wave)| *wave)
    .min();
  let launching = |issue: &Issue| {
    issue
      .pending
      .as_ref()
      .is_some_and(|pending| pending.action == Action::Launch)
  };
  let count = |wanted: &dyn Fn(&Issue) -> bool| issues.iter().filter(|issue| wanted(issue)).count();
  let live = count(&|issue| matches!(issue.stage.as_str(), "running" | "awaiting_merge"));
  let queued = count(&|issue| {
    issue.stage.is_empty() && !launching(issue) && !done.contains(issue.key.as_str())
  });
  let title = graph.and_then(|graph| cut(&secrets::redact_text(&graph.epic.title, secrets)));
  // A cut URL is a broken link, so a longer one is left out.
  let url = graph
    .map(|graph| secrets::redact_text(&graph.epic.url, secrets))
    .filter(|url| !url.is_empty() && url.chars().count() <= TEXT_LIMIT);
  let waves_total = waves
    .into_iter()
    .flat_map(BTreeMap::values)
    .collect::<BTreeSet<_>>()
    .len();
  let header = json!({
    "title": title, "url": url, "waves_total": waves_total,
    "current_wave": current, "live": live, "queued": queued,
  });
  header.as_object().cloned().unwrap_or_default()
}

/// Keys whose pull request merged (`merged`, or still `cleaning` or
/// `cleanup-incomplete`), that were abandoned, or closed before launch.
fn done_keys<'a>(issues: &[&'a Issue], rows: &'a [crate::graph::GraphIssue]) -> BTreeSet<&'a str> {
  let finished = issues.iter().filter(|issue| {
    matches!(
      issue.stage.as_str(),
      "merged" | "abandoned" | "cleaning" | "cleanup-incomplete"
    )
  });
  let closed = rows.iter().filter(|row| row.status == "done");
  finished
    .map(|issue| issue.key.as_str())
    .chain(closed.map(|row| row.key.as_str()))
    .collect()
}

/// `text` cut to 160 characters, or `None` when it is empty. Redact first: a
/// secret cut in two would no longer match.
fn cut(text: &str) -> Option<String> {
  (!text.is_empty()).then(|| text.chars().take(TEXT_LIMIT).collect())
}

/// One epic's safe graph, live issues and its merge-queue entries.
///
/// # Errors
/// The registry or graph cannot be read.
pub(crate) fn detail(
  world: &World,
  paths: &Paths,
  selector: &str,
  secrets: &Secrets,
) -> Result<Option<Value>, String> {
  let registered = registered(paths)?;
  let Some((key, dir)) = registered
    .iter()
    .find(|(key, _)| key == selector || key.ends_with(&format!("-{selector}")))
  else {
    return Ok(None);
  };
  let graph = read_value(&dir.join("graph.json"))?;
  let stable = world.epics.get(key).map(|epic| &epic.waves);
  let (nodes, waves) = project_graph(&graph, stable);
  let issues = epic_issues(world, key);
  let rows = issues
    .iter()
    .map(|issue| issue_row(issue, world.now_ms, secrets))
    .collect::<Vec<_>>();
  let mut value = json!({
    "schema": 1,
    "key": key,
    "paused": world.paused_all || world.paused.contains(key),
    "graph": {"issues": nodes, "waves": waves},
    "issues": rows,
    "queue": crate::queue::document(world, Some(key)).get("repositories").cloned(),
  });
  if let Some(map) = value.as_object_mut() {
    map.extend(header(world, key, &issues, secrets));
  }
  Ok(Some(value))
}

fn issue_row(issue: &Issue, now_ms: u64, secrets: &Secrets) -> Value {
  // One unreadable worker record only loses that row's fields, not the board.
  let record = |dir: &str| {
    read_value(
      &Path::new(&issue.state_dir)
        .join(dir)
        .join(format!("{}.json", issue.key)),
    )
    .unwrap_or(Value::Null)
  };
  let (meta, route) = (record("issues"), record("routes"));
  let (worktree, host) = crate::status_plan::locate(issue, &meta);
  let (model, effort) = crate::status_api::model_effort(&route, &meta);
  json!({
    "key": issue.key,
    "stage": issue.stage,
    "phase": issue.phase,
    "pr": issue.pr,
    "host": issue.kind,
    "agent_status": issue.agent_status,
    "agent_status_at": agent_status_at(issue),
    "model": model,
    "effort": effort,
    "phase_age_ms": now_ms.saturating_sub(issue.phase_at_ms),
    "hold": hold(issue, secrets),
    "plan_summary": crate::status_plan::plan_summary(worktree, host),
  })
}

/// When the issue's agent status last changed, as ISO UTC; `None` before any change.
pub(crate) fn agent_status_at(issue: &Issue) -> Option<String> {
  (issue.agent_status_at_ms > 0).then(|| {
    toolu_state::time::iso_millis(
      std::time::UNIX_EPOCH + std::time::Duration::from_millis(issue.agent_status_at_ms),
    )
  })
}

/// An issue's coupling hold as `{kind, with, at}`, or `null`; its `score` and
/// `source` stay private.
pub(crate) fn hold(issue: &Issue, secrets: &Secrets) -> Value {
  issue.hold.as_ref().map_or(Value::Null, |hold| {
    let text = |key: &str| {
      let text = hold.get(key).and_then(Value::as_str)?;
      cut(&secrets::redact_text(text, secrets))
    };
    json!({"kind": text("kind"), "with": text("with"), "at": text("at")})
  })
}

/// The graph's issues and waves. With the engine's stable waves, a merged
/// issue keeps its wave here too, matching `current_wave`.
fn project_graph(
  graph: &Value,
  stable: Option<&BTreeMap<String, u64>>,
) -> (Vec<Value>, Vec<Value>) {
  let mut nodes = Vec::new();
  let mut waves = BTreeMap::<u64, Vec<String>>::new();
  for row in graph
    .get("issues")
    .and_then(Value::as_array)
    .into_iter()
    .flatten()
  {
    let Some(key) = row.get("key").and_then(Value::as_str) else {
      continue;
    };
    let wave = stable
      .and_then(|waves| waves.get(key).copied())
      .or_else(|| row.get("wave").and_then(Value::as_u64));
    if let Some(wave) = wave {
      waves.entry(wave).or_default().push(key.to_owned());
    }
    nodes.push(json!({
      "key": key,
      "title": row.get("title").and_then(Value::as_str),
      "wave": wave,
      "open_blockers": safe_strings(row.get("open_blockers")),
    }));
  }
  let waves = waves
    .into_iter()
    .map(|(wave, keys)| json!({"wave": wave, "issues": keys}))
    .collect();
  (nodes, waves)
}

fn safe_strings(value: Option<&Value>) -> Vec<&str> {
  value
    .and_then(Value::as_array)
    .into_iter()
    .flatten()
    .filter_map(Value::as_str)
    .take(32)
    .collect()
}

#[cfg(test)]
#[path = "tests/status_epics_test.rs"]
mod tests;

#[cfg(test)]
#[path = "tests/status_epics_board_test.rs"]
mod board_tests;
