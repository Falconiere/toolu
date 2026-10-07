//! Append-only JSON lines under `<root>/journal/`, one file a day, with a
//! sequence number. A line without a trailing newline is ignored.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};
use toolu_state::time::{epoch_millis, iso_millis, iso_seconds};

use crate::lock::Held;
use crate::note::bounded_note;

/// One closed journal record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Record {
  /// Monotonic sequence.
  pub seq: u64,
  /// `transition`, `action`, `prompt`, `judgment`, `admission` or `recovery`.
  pub kind: String,
  /// What happened, for example `merge-intent`.
  pub name: String,
  /// Epic key, possibly empty.
  pub epic: String,
  /// Issue key, possibly empty.
  pub key: String,
  /// Action or spool token, possibly empty.
  pub token: String,
  /// Capped note.
  pub note: String,
}

impl Record {
  /// A record the caller fills in before `append` assigns `seq` and `at`.
  pub(crate) fn new(kind: &str, name: &str, key: &str, token: &str, note: &str) -> Self {
    Self {
      seq: 0,
      kind: kind.to_owned(),
      name: name.to_owned(),
      epic: String::new(),
      key: key.to_owned(),
      token: token.to_owned(),
      note: bounded_note(note),
    }
  }
}

/// Append `record` under `lock`, returning the sequence it was given.
///
/// # Errors
/// The lock is busy or the file cannot be written.
pub(crate) fn append(
  dir: &Path,
  lock: &Path,
  record: &Record,
  now: SystemTime,
) -> Result<Record, String> {
  let _held = Held::acquire(lock, "journal-busy")?;
  std::fs::create_dir_all(dir).map_err(|err| err.to_string())?;
  let seq = last_seq(dir, now)?.saturating_add(1);
  let mut stored = record.clone();
  stored.seq = seq;
  stored.note = bounded_note(&stored.note);
  let line = json!({
    "seq": seq,
    "at": iso_millis(now),
    "kind": stored.kind,
    "name": stored.name,
    "epic": stored.epic,
    "key": stored.key,
    "token": stored.token,
    "note": stored.note,
  });
  let mut text = serde_json::to_string(&line).map_err(|err| err.to_string())?;
  text.push('\n');
  let path = file_for(dir, now);
  let broken = std::fs::read(&path).is_ok_and(|bytes| !bytes.is_empty() && !bytes.ends_with(b"\n"));
  let mut file = std::fs::OpenOptions::new()
    .create(true)
    .append(true)
    .open(&path)
    .map_err(|err| err.to_string())?;
  if broken {
    std::io::Write::write_all(&mut file, b"\n").map_err(|err| err.to_string())?;
  }
  std::io::Write::write_all(&mut file, text.as_bytes()).map_err(|err| err.to_string())?;
  file.sync_all().map_err(|err| err.to_string())?;
  Ok(stored)
}

/// Complete records from today's file and the previous day's file, in order.
///
/// # Errors
/// A file cannot be read. A bad line is skipped.
pub(crate) fn tail(dir: &Path, now: SystemTime) -> Result<Vec<Record>, String> {
  let mut out = Vec::new();
  let previous = UNIX_EPOCH + Duration::from_secs(epoch_millis(now) / 1000);
  let yesterday = previous
    .checked_sub(Duration::from_hours(24))
    .unwrap_or(UNIX_EPOCH);
  for stamp in [yesterday, now] {
    out.extend(read_file(&dir.join(format!("{}.jsonl", day(stamp))))?);
  }
  Ok(out)
}

/// Delete daily files whose date is strictly older than `days` before `now`.
///
/// # Errors
/// The directory cannot be listed.
pub(crate) fn retain(dir: &Path, days: u32, now: SystemTime) -> Result<(), String> {
  let cutoff_secs =
    (epoch_millis(now) / 1000).saturating_sub(u64::from(days).saturating_mul(86_400));
  let cutoff = day(UNIX_EPOCH + Duration::from_secs(cutoff_secs));
  let entries = match std::fs::read_dir(dir) {
    Ok(entries) => entries,
    Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
    Err(err) => return Err(err.to_string()),
  };
  for entry in entries {
    let path = entry.map_err(|err| err.to_string())?.path();
    let Some(name) = file_name(&path) else {
      continue;
    };
    let Some(stem) = name.strip_suffix(".jsonl") else {
      continue;
    };
    if stem < cutoff.as_str() {
      let _gone = std::fs::remove_file(path);
    }
  }
  Ok(())
}

fn day(now: SystemTime) -> String {
  iso_seconds(now).chars().take(10).collect()
}

fn file_name(path: &Path) -> Option<String> {
  path
    .file_name()
    .map(|name| name.to_string_lossy().into_owned())
}

fn last_seq(dir: &Path, now: SystemTime) -> Result<u64, String> {
  let rows = read_file(&dir.join(format!("{}.jsonl", day(now))))?;
  Ok(rows.last().map_or(0, |row| row.seq))
}

fn read_file(path: &Path) -> Result<Vec<Record>, String> {
  let text = match std::fs::read_to_string(path) {
    Ok(text) => text,
    Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
    Err(err) => return Err(err.to_string()),
  };
  let mut rows = Vec::new();
  for line in text.lines() {
    if let Some(row) = parse(line) {
      rows.push(row);
    }
  }
  Ok(rows)
}

fn parse(line: &str) -> Option<Record> {
  let value: Value = serde_json::from_str(line).ok()?;
  let field = |key: &str| {
    value
      .get(key)
      .and_then(Value::as_str)
      .unwrap_or("")
      .to_owned()
  };
  let seq = value.get("seq").and_then(Value::as_u64)?;
  Some(Record {
    seq,
    kind: field("kind"),
    name: field("name"),
    epic: field("epic"),
    key: field("key"),
    token: field("token"),
    note: field("note"),
  })
}

/// The path of a journal file, exposed for tests that plant a partial line.
pub(crate) fn file_for(dir: &Path, now: SystemTime) -> PathBuf {
  dir.join(format!("{}.jsonl", day(now)))
}

#[cfg(test)]
#[path = "tests/journal_test.rs"]
mod tests;
