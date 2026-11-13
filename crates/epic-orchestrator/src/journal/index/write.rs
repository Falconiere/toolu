//! Sync JSONL records before the cache is updated.

use std::io::Write;
use std::path::Path;
use std::time::SystemTime;

use serde_json::json;

use super::super::{Record, file_for};
use super::{Placed, Summary, len_of};

/// Append every record, then sync the day file once.
///
/// # Errors
/// The daily file cannot be written.
pub(super) fn write_lines(
  dir: &Path,
  now: SystemTime,
  records: &[Record],
  summary: &Summary,
) -> Result<Vec<Placed>, String> {
  if records.is_empty() {
    return Ok(Vec::new());
  }
  let path = file_for(dir, now);
  let day = super::super::day(now);
  let len = len_of(&path)?;
  let torn = summary
    .files
    .iter()
    .any(|row| row.date == day && len > row.end);
  let mut file = std::fs::OpenOptions::new()
    .create(true)
    .append(true)
    .open(&path)
    .map_err(|err| err.to_string())?;
  if torn {
    file.write_all(b"\n").map_err(|err| err.to_string())?;
  }
  let mut out = Vec::with_capacity(records.len());
  for record in records {
    let start = file.metadata().map_err(|err| err.to_string())?.len();
    file
      .write_all(line_text(record)?.as_bytes())
      .map_err(|err| err.to_string())?;
    let end = file.metadata().map_err(|err| err.to_string())?.len();
    out.push(Placed {
      day: day.clone(),
      start,
      end,
      torn: torn && out.is_empty(),
    });
  }
  super::flush(&file)?;
  super::batch_event("flushed");
  Ok(out)
}

#[cfg(test)]
#[path = "tests/write_test.rs"]
mod tests;

fn line_text(record: &Record) -> Result<String, String> {
  let line = json!({
    "seq": record.seq,
    "at": record.at,
    "kind": record.kind,
    "name": record.name,
    "epic": record.epic,
    "key": record.key,
    "token": record.token,
    "note": record.note,
  });
  let mut text = serde_json::to_string(&line).map_err(|err| err.to_string())?;
  text.push('\n');
  Ok(text)
}
