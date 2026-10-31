//! Rebuildable sequence cache for the daily JSONL log.
//!
//! `sequence.index` is the committed summary. `sequence.offsets` is an ASCII
//! table of each complete line. The JSONL record is synced before either
//! update. A missing, stale, or corrupt cache is rebuilt from the log.

use std::cell::RefCell;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

use toolu_state::time::iso_millis;

use super::Record;

mod read;
mod scan;
mod write;

const HEADER: &[u8] = b"TJO1\n";
const ROW: u64 = 41;

thread_local! {
  /// Per-thread JSONL read counter. Absent unless a test installs one.
  pub(crate) static SINK: RefCell<Option<Arc<AtomicU64>>> = const { RefCell::new(None) };
}

#[derive(serde::Serialize, serde::Deserialize)]
struct Day {
  #[serde(rename = "day")]
  date: String,
  first: u64,
  last: u64,
  end: u64,
  count: u64,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct Summary {
  last: u64,
  oldest: Option<u64>,
  records: u64,
  files: Vec<Day>,
}

struct Off {
  seq: u64,
  file: u32,
  pos: u64,
}

struct Placed {
  day: String,
  start: u64,
  end: u64,
  torn: bool,
}

fn note(bytes: usize) {
  let Ok(count) = u64::try_from(bytes) else {
    return;
  };
  SINK.with(|slot| {
    if let Some(sink) = slot.borrow().as_ref() {
      let _prev = sink.fetch_add(count, Ordering::Relaxed);
    }
  });
}

fn as_u64(value: usize) -> Result<u64, String> {
  u64::try_from(value).map_err(|err| err.to_string())
}

fn summary_path(dir: &Path) -> PathBuf {
  dir.join("sequence.index")
}

fn offsets_path(dir: &Path) -> PathBuf {
  dir.join("sequence.offsets")
}

fn parsed(line: &[u8]) -> Option<Record> {
  super::parse(std::str::from_utf8(line).ok()?)
}

/// Assign the next sequence, sync the JSONL line, then update the cache.
///
/// # Errors
/// The log or the cache cannot be written.
pub(super) fn append_locked(
  dir: &Path,
  record: &mut Record,
  now: SystemTime,
) -> Result<(), String> {
  let summary = fresh(dir)?;
  record.seq = summary.last.saturating_add(1);
  record.at = iso_millis(now);
  let placed = write::write_line(dir, now, record, &summary)?;
  if placed.torn {
    return scan::rebuild(dir).map(|_| ());
  }
  publish(dir, summary, record, &placed)
}

/// Records strictly after `since`, and the oldest retained sequence.
///
/// # Errors
/// The log or the cache cannot be read.
pub(super) fn page(
  dir: &Path,
  lock: &Path,
  since: u64,
  limit: usize,
) -> Result<(Option<u64>, Vec<Record>), String> {
  if !dir.exists() {
    return Ok((None, Vec::new()));
  }
  match read_page(dir, lock, since, limit) {
    Err(err) if err.starts_with("corrupt journal index") => {
      let _held = super::journal_held(lock)?;
      scan::rebuild(dir)?;
      read_page(dir, lock, since, limit)
    }
    other => other,
  }
}

fn read_page(
  dir: &Path,
  lock: &Path,
  since: u64,
  limit: usize,
) -> Result<(Option<u64>, Vec<Record>), String> {
  let summary = ensure(dir, lock)?;
  let rows = read::collect(dir, &summary, since, limit)?;
  Ok((summary.oldest, rows))
}

fn ensure(dir: &Path, lock: &Path) -> Result<Summary, String> {
  if let Some(summary) = ready(dir)? {
    return Ok(summary);
  }
  let _held = super::journal_held(lock)?;
  fresh(dir)
}

fn fresh(dir: &Path) -> Result<Summary, String> {
  if let Some(summary) = ready(dir)? {
    return Ok(summary);
  }
  scan::rebuild(dir)
}

fn ready(dir: &Path) -> Result<Option<Summary>, String> {
  let Some(summary) = load(dir)? else {
    return Ok(None);
  };
  if scan::consistent(dir, &summary)? {
    return Ok(Some(summary));
  }
  Ok(None)
}

fn load(dir: &Path) -> Result<Option<Summary>, String> {
  match std::fs::read(summary_path(dir)) {
    Ok(bytes) => Ok(serde_json::from_slice(&bytes).ok()),
    Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
    Err(err) => Err(err.to_string()),
  }
}

fn store(dir: &Path, summary: &Summary) -> Result<(), String> {
  let path = summary_path(dir);
  let tmp = dir.join(format!(".sequence.index.{}.tmp", std::process::id()));
  let text = serde_json::to_string(summary).map_err(|err| err.to_string())?;
  write_synced(&tmp, text.as_bytes())?;
  std::fs::rename(&tmp, &path).map_err(|err| err.to_string())?;
  File::open(dir)
    .and_then(|file| file.sync_all())
    .map_err(|err| err.to_string())
}

fn write_synced(path: &Path, bytes: &[u8]) -> Result<(), String> {
  let mut file = std::fs::OpenOptions::new()
    .write(true)
    .create(true)
    .truncate(true)
    .open(path)
    .map_err(|err| err.to_string())?;
  file.write_all(bytes).map_err(|err| err.to_string())?;
  file.sync_all().map_err(|err| err.to_string())
}

fn len_of(path: &Path) -> Result<u64, String> {
  match path.metadata() {
    Ok(meta) => Ok(meta.len()),
    Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(0),
    Err(err) => Err(err.to_string()),
  }
}

fn publish(
  dir: &Path,
  mut summary: Summary,
  record: &Record,
  placed: &Placed,
) -> Result<(), String> {
  let file_id = touch_day(&mut summary, &placed.day, record.seq, placed.end)?;
  let mut file = std::fs::OpenOptions::new()
    .create(true)
    .append(true)
    .open(offsets_path(dir))
    .map_err(|err| err.to_string())?;
  write_off(
    &mut file,
    &Off {
      seq: record.seq,
      file: file_id,
      pos: placed.start,
    },
  )?;
  file.sync_all().map_err(|err| err.to_string())?;
  summary.records = summary.records.saturating_add(1);
  summary.last = record.seq;
  if summary.oldest.is_none() {
    summary.oldest = Some(record.seq);
  }
  store(dir, &summary)
}

fn touch_day(summary: &mut Summary, day: &str, seq: u64, end: u64) -> Result<u32, String> {
  if let Some(index) = summary.files.iter().position(|row| row.date == day) {
    let Some(row) = summary.files.get_mut(index) else {
      return Err("corrupt journal index".to_owned());
    };
    if row.count == 0 {
      row.first = seq;
    }
    row.last = seq;
    row.end = end;
    row.count = row.count.saturating_add(1);
    return u32::try_from(index).map_err(|err| err.to_string());
  }
  let index = u32::try_from(summary.files.len()).map_err(|err| err.to_string())?;
  summary.files.push(Day {
    date: day.to_owned(),
    first: seq,
    last: seq,
    end,
    count: 1,
  });
  Ok(index)
}

fn write_off(file: &mut File, hit: &Off) -> Result<(), String> {
  let line = format!("{:016x}{:08x}{:016x}\n", hit.seq, hit.file, hit.pos);
  file
    .write_all(line.as_bytes())
    .map_err(|err| err.to_string())
}

#[cfg(test)]
#[path = "tests/index_test.rs"]
mod tests;
