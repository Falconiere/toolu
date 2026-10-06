//! The documents the gate-file writers build (`gate-file.ts`): the seed of
//! the previous slots, the failing document after a record, the document
//! after a clear, and the text both implementations write. Slots sort by
//! `(updatedAt, key)` in byte order, as jq's `sort_by` does, oldest first.

use toolu_runtime::json::jq_text;
use toolu_runtime::json::ordered::Ordered;

use crate::gate_schema::{GLOBAL_GATE_KEY, GateEntry, GateFile};
use crate::js_order::js_order;

/// One failure being recorded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Failure<'a> {
  pub(crate) file: &'a str,
  pub(crate) source: &'a str,
  pub(crate) reason: &'a str,
  pub(crate) violations: &'a str,
  pub(crate) now: &'a str,
}

/// The slots both writers start from: `entries`, else the legacy single slot.
pub(crate) fn seed_entries(doc: &GateFile) -> Vec<(String, GateEntry)> {
  match doc {
    GateFile::Passing { .. } => Vec::new(),
    GateFile::Failing {
      entries: Some(entries),
      ..
    } => entries.clone(),
    GateFile::Failing {
      reason,
      source,
      file,
      violations,
      entries: None,
      updated_at,
    } => {
      let entry = GateEntry {
        source: source.clone(),
        reason: reason.clone(),
        violations: violations.clone(),
        updated_at: updated_at.clone(),
      };
      vec![(file.clone(), entry)]
    }
  }
}

/// Oldest first by `(updatedAt, key)`, byte order.
fn sorted(entries: &[(String, GateEntry)]) -> Vec<&(String, GateEntry)> {
  let mut sorted: Vec<&(String, GateEntry)> = entries.iter().collect();
  sorted.sort_by(|(ka, a), (kb, b)| a.updated_at.cmp(&b.updated_at).then_with(|| ka.cmp(kb)));
  sorted
}

fn joined(sorted: &[&(String, GateEntry)]) -> String {
  sorted
    .iter()
    .map(|(_, entry)| entry.violations.as_str())
    .collect()
}

/// `{ ...prev, [file]: entry }`: replaced in place, else appended, in JS order.
pub(crate) fn failing_doc(prev: Vec<(String, GateEntry)>, f: &Failure<'_>) -> GateFile {
  let entry = GateEntry {
    source: f.source.to_owned(),
    reason: f.reason.to_owned(),
    violations: f.violations.to_owned(),
    updated_at: f.now.to_owned(),
  };
  let mut entries = prev;
  match entries.iter_mut().find(|(key, _)| key == f.file) {
    Some(slot) => slot.1 = entry,
    None => entries.push((f.file.to_owned(), entry)),
  }
  let entries = js_order(entries);
  GateFile::Failing {
    reason: f.reason.to_owned(),
    source: f.source.to_owned(),
    file: f.file.to_owned(),
    violations: joined(&sorted(&entries)),
    entries: Some(entries),
    updated_at: f.now.to_owned(),
  }
}

/// After a clear: the latest remaining slot is promoted; none left is passing.
pub(crate) fn cleared_doc(left: Vec<(String, GateEntry)>, by: &str, now: &str) -> GateFile {
  let order = sorted(&left);
  let Some((key, latest)) = order.last().copied() else {
    return GateFile::Passing {
      source: by.to_owned(),
      updated_at: now.to_owned(),
    };
  };
  let (reason, source, file) = (latest.reason.clone(), latest.source.clone(), key.clone());
  let violations = joined(&order);
  GateFile::Failing {
    reason,
    source,
    file,
    violations,
    entries: Some(left),
    updated_at: now.to_owned(),
  }
}

/// What the writers put in the file: two-space jq text and a newline.
pub(crate) fn doc_text(doc: &Ordered) -> String {
  format!("{}\n", jq_text(doc, true))
}

/// The single-slot record the fallback writes when the atomic write fails.
pub(crate) fn single_slot(f: &Failure<'_>) -> Ordered {
  let text = |value: &str| Ordered::String(value.to_owned());
  Ordered::Object(vec![
    ("status".to_owned(), text("failing")),
    ("reason".to_owned(), text(f.reason)),
    ("source".to_owned(), text(f.source)),
    ("file".to_owned(), text(f.file)),
    ("violations".to_owned(), text(f.violations)),
    ("updatedAt".to_owned(), text(f.now)),
  ])
}

/// Slots a replaced document held besides `file`, counted with the seed rule.
pub(crate) fn dropped_count(value: &Ordered, file: &str) -> usize {
  let keys: Vec<String> = if let Some(Ordered::Object(slots)) = value.get("entries") {
    slots.iter().map(|(key, _)| key.clone()).collect()
  } else if value.get("status") == Some(&Ordered::String("failing".to_owned())) {
    let named = if let Some(Ordered::String(named)) = value.get("file") {
      named.as_str()
    } else {
      GLOBAL_GATE_KEY
    };
    vec![named.to_owned()]
  } else {
    Vec::new()
  };
  keys.iter().filter(|key| key.as_str() != file).count()
}

#[cfg(test)]
#[path = "tests/gate_doc_test.rs"]
mod tests;
