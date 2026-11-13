//! Append-only JSON lines under `<root>/journal/`, one file a day, with a
//! sequence number. A line without a trailing newline is ignored.
//! `sequence.index` and `sequence.offsets` are a rebuildable cache of the last
//! sequence and each complete line's offset. A single append syncs the JSONL
//! line once; a consecutive engine batch syncs once after every line of the
//! batch.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::de::{IgnoredAny, MapAccess, SeqAccess, Visitor};
use toolu_state::time::{epoch_millis, iso_seconds};

/// Rebuildable sequence and offset cache for the daily JSONL log.
pub(crate) mod index;

use crate::lock::Held;
use crate::note::bounded_note;

/// One closed journal record.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
pub(crate) struct Record {
  /// Monotonic sequence.
  pub seq: u64,
  /// UTC journal timestamp.
  #[serde(default, deserialize_with = "text_or_empty")]
  pub at: String,
  /// `transition`, `action`, `prompt`, `judgment`, `admission`, `recovery` or
  /// `notify` (a worker notice).
  #[serde(default, deserialize_with = "text_or_empty")]
  pub kind: String,
  /// What happened, for example `merge-intent`.
  #[serde(default, deserialize_with = "text_or_empty")]
  pub name: String,
  /// Epic key, possibly empty.
  #[serde(default, deserialize_with = "text_or_empty")]
  pub epic: String,
  /// Issue key, possibly empty.
  #[serde(default, deserialize_with = "text_or_empty")]
  pub key: String,
  /// Action or spool token, possibly empty.
  #[serde(default, deserialize_with = "text_or_empty")]
  pub token: String,
  /// Capped note.
  #[serde(default, deserialize_with = "text_or_empty")]
  pub note: String,
}

fn text_or_empty<'de, D>(deserializer: D) -> Result<String, D::Error>
where
  D: serde::Deserializer<'de>,
{
  deserializer.deserialize_any(Text)
}

struct Text;

impl<'de> Visitor<'de> for Text {
  type Value = String;

  fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.write_str("a string or any ignored JSON value")
  }

  fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<String, E> {
    Ok(value.to_owned())
  }

  fn visit_string<E: serde::de::Error>(self, value: String) -> Result<String, E> {
    Ok(value)
  }

  fn visit_bool<E: serde::de::Error>(self, _value: bool) -> Result<String, E> {
    Ok(String::new())
  }

  fn visit_i64<E: serde::de::Error>(self, _value: i64) -> Result<String, E> {
    Ok(String::new())
  }

  fn visit_u64<E: serde::de::Error>(self, _value: u64) -> Result<String, E> {
    Ok(String::new())
  }

  fn visit_f64<E: serde::de::Error>(self, _value: f64) -> Result<String, E> {
    Ok(String::new())
  }

  fn visit_unit<E: serde::de::Error>(self) -> Result<String, E> {
    Ok(String::new())
  }

  fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<String, A::Error> {
    while seq.next_element::<IgnoredAny>()?.is_some() {}
    Ok(String::new())
  }

  fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<String, A::Error> {
    while map.next_entry::<IgnoredAny, IgnoredAny>()?.is_some() {}
    Ok(String::new())
  }
}

impl Record {
  /// A record the caller fills in before `append` assigns `seq` and `at`.
  pub(crate) fn new(kind: &str, name: &str, key: &str, token: &str, note: &str) -> Self {
    Self {
      seq: 0,
      at: String::new(),
      kind: kind.to_owned(),
      name: name.to_owned(),
      epic: String::new(),
      key: key.to_owned(),
      token: token.to_owned(),
      note: bounded_note(note),
    }
  }
}

/// How long the state loop retries a live holder of the journal lock.
const LOCK_WAIT: Duration = Duration::from_secs(2);

/// Take the journal lock, retrying a live holder for `wait`.
pub(super) fn journal_held(lock: &Path, wait: Duration) -> Result<Held, String> {
  let start = std::time::Instant::now();
  loop {
    match Held::acquire(lock, "journal-busy") {
      Ok(held) => return Ok(held),
      Err(err) if err == "journal-busy" && start.elapsed() < wait => {
        std::thread::sleep(Duration::from_millis(20));
      }
      Err(err) => return Err(err),
    }
  }
}

/// Append `record` under `lock`, returning the sequence it was given.
///
/// # Errors
/// The lock is busy past [`LOCK_WAIT`] or the file cannot be written.
pub(crate) fn append(
  dir: &Path,
  lock: &Path,
  record: &Record,
  now: SystemTime,
) -> Result<Record, String> {
  append_within(dir, lock, record, now, LOCK_WAIT)
}

/// [`append`] retrying a live holder for `wait` instead: a thread nothing
/// waits on can afford more than the state loop.
///
/// # Errors
/// The lock is busy past `wait` or the file cannot be written.
pub(crate) fn append_within(
  dir: &Path,
  lock: &Path,
  record: &Record,
  now: SystemTime,
  wait: Duration,
) -> Result<Record, String> {
  let mut stored = append_batch_within(dir, lock, std::slice::from_ref(record), now, wait)?;
  stored
    .pop()
    .ok_or_else(|| "missing journal record".to_owned())
}

/// Append every record under one lock hold and one JSONL sync.
///
/// # Errors
/// The lock is busy past [`LOCK_WAIT`] or the file cannot be written.
pub(crate) fn append_batch(
  dir: &Path,
  lock: &Path,
  records: &[Record],
  now: SystemTime,
) -> Result<Vec<Record>, String> {
  append_batch_within(dir, lock, records, now, LOCK_WAIT)
}

/// [`append_batch`] retrying a live holder for `wait` instead.
///
/// # Errors
/// The lock is busy past `wait` or the file cannot be written.
pub(crate) fn append_batch_within(
  dir: &Path,
  lock: &Path,
  records: &[Record],
  now: SystemTime,
  wait: Duration,
) -> Result<Vec<Record>, String> {
  if records.is_empty() {
    return Ok(Vec::new());
  }
  let _held = journal_held(lock, wait)?;
  std::fs::create_dir_all(dir).map_err(|err| err.to_string())?;
  let mut stored: Vec<Record> = records
    .iter()
    .map(|record| {
      let mut row = record.clone();
      row.note = bounded_note(&row.note);
      row
    })
    .collect();
  index::append_batch_locked(dir, &mut stored, now)?;
  Ok(stored)
}

/// Record a batch phase label when a test sink is installed.
pub(crate) fn batch_event(label: &'static str) {
  index::batch_event(label);
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
    out.extend(read_file(&dir.join(format!("{}.jsonl", day(stamp))), "")?);
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

/// `YYYY-MM-DD` for `now`.
pub(super) fn day(now: SystemTime) -> String {
  iso_seconds(now).chars().take(10).collect()
}

fn file_name(path: &Path) -> Option<String> {
  path
    .file_name()
    .map(|name| name.to_string_lossy().into_owned())
}

fn read_file(path: &Path, needle: &str) -> Result<Vec<Record>, String> {
  let text = match std::fs::read_to_string(path) {
    Ok(text) => text,
    Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
    Err(err) => return Err(err.to_string()),
  };
  let mut rows = Vec::new();
  for line in text
    .split_inclusive('\n')
    .filter(|line| line.ends_with('\n'))
    .filter(|line| line.contains(needle))
  {
    if let Some(row) = parse(line.trim_end_matches('\n')) {
      rows.push(row);
    }
  }
  Ok(rows)
}

/// One complete JSON object, or nothing when `line` is not a record.
pub(super) fn parse(line: &str) -> Option<Record> {
  serde_json::from_str(line).ok()
}

/// Retained records after `since`, in sequence order, and the oldest retained sequence.
///
/// # Errors
/// A journal directory or file cannot be read.
pub(crate) fn page(
  dir: &Path,
  lock: &Path,
  since: u64,
  limit: usize,
) -> Result<(Option<u64>, Vec<Record>), String> {
  index::page(dir, lock, since, limit)
}

/// Hand every retained record from files dated `from_day` (`YYYY-MM-DD`) or
/// later to `each`, file by file in date order, so a reader never holds the
/// whole journal.
///
/// # Errors
/// The directory or a file cannot be read, or `each` fails.
pub(crate) fn each<F>(dir: &Path, from_day: &str, each: F) -> Result<(), String>
where
  F: FnMut(Record) -> Result<(), String>,
{
  each_with(dir, from_day, "", each)
}

/// Like [`each`], skipping lines without the JSON-encoded token text before parsing.
///
/// # Errors
/// The directory or a file cannot be read, or `each` fails.
pub(crate) fn each_with<F>(
  dir: &Path,
  from_day: &str,
  needle: &str,
  mut each: F,
) -> Result<(), String>
where
  F: FnMut(Record) -> Result<(), String>,
{
  let encoded = if needle.is_empty() {
    String::new()
  } else {
    serde_json::to_string(needle).map_err(|err| err.to_string())?
  };
  let entries = match std::fs::read_dir(dir) {
    Ok(entries) => entries,
    Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
    Err(err) => return Err(err.to_string()),
  };
  let mut days = Vec::new();
  for entry in entries {
    let path = entry.map_err(|err| err.to_string())?.path();
    let Some(name) = file_name(&path) else {
      continue;
    };
    if let Some(stem) = name.strip_suffix(".jsonl")
      && stem >= from_day
    {
      days.push(path);
    }
  }
  days.sort();
  for path in days {
    for row in read_file(&path, &encoded)? {
      each(row)?;
    }
  }
  Ok(())
}

/// The path of a journal file, exposed for tests that plant a partial line.
pub(crate) fn file_for(dir: &Path, now: SystemTime) -> PathBuf {
  dir.join(format!("{}.jsonl", day(now)))
}

#[cfg(test)]
#[path = "tests/journal_test.rs"]
mod tests;
