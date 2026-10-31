//! Public, versioned journal projection for the API and event stream.

use serde_json::{Value, json};
use toolu_runtime::config::secrets::{self, Secrets};

use crate::journal::{self, Record};
use crate::paths::Paths;

/// A page of retained events strictly after `since`.
///
/// # Errors
/// The journal cannot be read.
pub(crate) fn page(
  paths: &Paths,
  since: u64,
  limit: usize,
  secrets: &Secrets,
) -> Result<Value, String> {
  let (oldest, rows) = journal::page(&paths.journal_dir(), &paths.journal_lock(), since, limit)?;
  let next = rows.last().map_or(since, |row| row.seq);
  let events: Vec<Value> = rows.iter().map(|row| public_event(row, secrets)).collect();
  Ok(json!({"schema": 1, "events": events, "oldest_seq": oldest, "next_seq": next}))
}

fn public_event(row: &Record, secrets: &Secrets) -> Value {
  let value = json!({
    "seq": row.seq,
    "at": row.at,
    "kind": row.kind,
    "name": row.name,
    "epic": row.epic,
    "key": row.key,
    "note": row.note.chars().take(160).collect::<String>(),
  });
  secrets::redact_json(&value, secrets)
}

#[cfg(test)]
#[path = "tests/status_events_test.rs"]
mod tests;
