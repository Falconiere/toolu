//! The metrics document: one pass learns the scope, a second folds every
//! in-scope record into the timeline, the ready split, the counts and the
//! GitHub figures.

use std::path::Path;
use std::time::{Duration, UNIX_EPOCH};

use serde_json::{Map, Value, json};
use toolu_state::time::{epoch_millis, iso_millis, iso_seconds, parse_iso};

use crate::journal::{self, Record};
use crate::metrics::Query;
use crate::metrics::counts::Counts;
use crate::metrics::github::GitHub;
use crate::metrics::ready::Ready;
use crate::metrics::scope::Scope;
use crate::metrics::stats::{pct, share};
use crate::metrics::timeline::Timeline;

#[derive(Default)]
struct Fold {
  records: u64,
  timeline: Timeline,
  ready: Ready,
  counts: Counts,
  github: GitHub,
}

/// The schema-1 metrics document of the journal in `dir`.
///
/// # Errors
/// The journal cannot be read.
pub(crate) fn document(dir: &Path, query: &Query) -> Result<Value, String> {
  let from_day = query.since_ms.map_or_else(String::new, |since| {
    iso_seconds(UNIX_EPOCH + Duration::from_millis(since))
      .chars()
      .take(10)
      .collect()
  });
  let mut scope = Scope::new(query.epic.as_deref());
  journal::each(dir, &from_day, |record| {
    scope.learn(&record);
    Ok(())
  })?;
  let mut fold = Fold::default();
  journal::each(dir, &from_day, |record| {
    push(&mut fold, &scope, query, &record);
    Ok(())
  })?;
  Ok(finish(fold, query))
}

fn push(fold: &mut Fold, scope: &Scope, query: &Query, record: &Record) {
  let Some(at) = parse_iso(&record.at).map(epoch_millis) else {
    return;
  };
  if query.since_ms.is_some_and(|since| at < since) || !scope.keeps(record) {
    return;
  }
  fold.records = fold.records.saturating_add(1);
  let issue = scope.issue(&record.key).unwrap_or("");
  fold.timeline.push(issue, record, at);
  fold.ready.push(issue, record, at);
  fold.counts.push(issue, record, at);
  fold.github.push(scope, issue, record, at);
}

fn finish(fold: Fold, query: &Query) -> Value {
  let times = fold.timeline.finish(query.now_ms);
  let split = fold.ready.finish_all();
  let total: u64 = times.phases.values().sum();
  let phases: Map<String, Value> = times
    .phases
    .iter()
    .map(|(phase, ms)| (phase.clone(), json!({"ms": ms, "share": share(*ms, total)})))
    .collect();
  let [queue, rebase, ci, merge, rework] = &split.buckets;
  let mut doc = json!({
    "schema": 1,
    "epic": query.epic,
    "since": query.since_ms.map(|ms| iso_millis(UNIX_EPOCH + Duration::from_millis(ms))),
    "generated_at": iso_millis(UNIX_EPOCH + Duration::from_millis(query.now_ms)),
    "records": fold.records,
    "issues": times.issues,
    "lead_time_ms": pct(&times.lead),
    "phases": phases,
    "ready_to_merged_ms": {
      "total": pct(&split.total), "queue": pct(queue), "rebase": pct(rebase), "ci": pct(ci),
      "merge": pct(merge), "rework": pct(rework),
      "clean_excluding_ci": pct(&split.clean_excluding_ci),
    },
    "rebase_rounds": fold.counts.rounds(),
    "github": fold.github.finish(),
  });
  if let (Some(doc), Value::Object(counts)) =
    (doc.as_object_mut(), fold.counts.finish(times.relaunches))
  {
    doc.extend(counts);
  }
  doc
}

#[cfg(test)]
#[path = "tests/document_test.rs"]
mod tests;
