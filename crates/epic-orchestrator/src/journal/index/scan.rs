//! Rebuild the sequence cache from the JSONL files.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use super::{
  Day, HEADER, Off, Summary, as_u64, len_of, note, offsets_path, parsed, store, write_off,
};

/// Replace the cache with one scan of the daily logs.
pub(super) fn rebuild(dir: &Path) -> Result<Summary, String> {
  let mut files = Vec::new();
  let mut hits = Vec::new();
  for path in list_days(dir)? {
    push_file(&path, &mut files, &mut hits)?;
  }
  hits.sort_by_key(|hit: &Off| hit.seq);
  write_offsets(dir, &hits)?;
  let summary = Summary {
    last: hits.last().map_or(0, |hit| hit.seq),
    oldest: hits.first().map(|hit| hit.seq),
    records: as_u64(hits.len())?,
    files,
  };
  store(dir, &summary)?;
  Ok(summary)
}

/// Whether `summary` still matches the logs and the offset table.
pub(super) fn consistent(dir: &Path, summary: &Summary) -> Result<bool, String> {
  if !totals_ok(summary) || !offsets_match(dir, summary.records)? {
    return Ok(false);
  }
  let days = list_days(dir)?;
  if days.len() != summary.files.len() {
    return Ok(false);
  }
  for (day, path) in summary.files.iter().zip(days) {
    if stem_of(&path)? != day.date || len_of(&path)? < day.end || extra_record(&path, day.end)? {
      return Ok(false);
    }
  }
  Ok(true)
}

fn totals_ok(summary: &Summary) -> bool {
  let records = summary.files.iter().map(|day| day.count).sum::<u64>();
  let live: Vec<&Day> = summary.files.iter().filter(|day| day.count > 0).collect();
  if records != summary.records {
    return false;
  }
  let Some(min) = live.iter().map(|day| day.first).min() else {
    return summary.last == 0 && summary.oldest.is_none();
  };
  let max = live.iter().map(|day| day.last).max().unwrap_or(0);
  summary.oldest == Some(min) && summary.last == max
}

fn offsets_match(dir: &Path, records: u64) -> Result<bool, String> {
  let Some(expected) = records
    .checked_mul(super::ROW)
    .and_then(|rows| rows.checked_add(5))
  else {
    return Ok(false);
  };
  let path = offsets_path(dir);
  let len = match path.metadata() {
    Ok(meta) => meta.len(),
    Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(false),
    Err(err) => return Err(err.to_string()),
  };
  if len != expected {
    return Ok(false);
  }
  let mut header = [0u8; 5];
  let mut file = File::open(path).map_err(|err| err.to_string())?;
  file
    .read_exact(&mut header)
    .map_err(|err| err.to_string())?;
  Ok(header.as_slice() == HEADER)
}

fn list_days(dir: &Path) -> Result<Vec<PathBuf>, String> {
  let entries = match std::fs::read_dir(dir) {
    Ok(entries) => entries,
    Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
    Err(err) => return Err(err.to_string()),
  };
  let mut days = Vec::new();
  for entry in entries {
    let path = entry.map_err(|err| err.to_string())?.path();
    if path.extension().is_some_and(|ext| ext == "jsonl") {
      days.push(path);
    }
  }
  days.sort();
  Ok(days)
}

fn stem_of(path: &Path) -> Result<String, String> {
  path
    .file_stem()
    .and_then(|stem| stem.to_str())
    .map(ToOwned::to_owned)
    .ok_or_else(|| format!("journal name {}", path.display()))
}

#[cfg(test)]
#[path = "tests/scan_test.rs"]
mod tests;

fn read_rest(file: &mut File, buf: &mut Vec<u8>) -> Result<(), String> {
  let mut chunk = [0u8; 256];
  loop {
    let n = file.read(&mut chunk).map_err(|err| err.to_string())?;
    if n == 0 {
      return Ok(());
    }
    let Some(got) = chunk.get(..n) else {
      return Ok(());
    };
    buf.extend_from_slice(got);
  }
}

fn extra_record(path: &Path, end: u64) -> Result<bool, String> {
  let len = len_of(path)?;
  if len <= end {
    return Ok(false);
  }
  let mut file = File::open(path).map_err(|err| err.to_string())?;
  file
    .seek(SeekFrom::Start(end))
    .map_err(|err| err.to_string())?;
  let mut buf = Vec::new();
  read_rest(&mut file, &mut buf)?;
  note(buf.len());
  Ok(has_record(&buf))
}

fn has_record(bytes: &[u8]) -> bool {
  let mut start = 0usize;
  for (index, byte) in bytes.iter().enumerate() {
    if *byte != b'\n' {
      continue;
    }
    if let Some(line) = bytes.get(start..index)
      && parsed(line).is_some()
    {
      return true;
    }
    start = index.saturating_add(1);
  }
  false
}

fn push_file(path: &Path, files: &mut Vec<Day>, hits: &mut Vec<Off>) -> Result<(), String> {
  let file_id = u32::try_from(files.len()).map_err(|err| err.to_string())?;
  let (rows, end) = scan_file(path)?;
  let count = as_u64(rows.len())?;
  let first = rows.iter().map(|row| row.0).min().unwrap_or(0);
  let last = rows.iter().map(|row| row.0).max().unwrap_or(0);
  files.push(Day {
    date: stem_of(path)?,
    first,
    last,
    end,
    count,
  });
  for (seq, pos) in rows {
    hits.push(Off {
      seq,
      file: file_id,
      pos,
    });
  }
  Ok(())
}

fn scan_file(path: &Path) -> Result<(Vec<(u64, u64)>, u64), String> {
  let bytes = std::fs::read(path).map_err(|err| err.to_string())?;
  note(bytes.len());
  let mut rows = Vec::new();
  let mut start = 0usize;
  for (index, byte) in bytes.iter().enumerate() {
    if *byte != b'\n' {
      continue;
    }
    if let Some(line) = bytes.get(start..index)
      && let Some(row) = parsed(line)
    {
      rows.push((row.seq, as_u64(start)?));
    }
    start = index.saturating_add(1);
  }
  Ok((rows, as_u64(start)?))
}

fn write_offsets(dir: &Path, hits: &[Off]) -> Result<(), String> {
  let path = offsets_path(dir);
  let mut file = std::fs::OpenOptions::new()
    .write(true)
    .create(true)
    .truncate(true)
    .open(&path)
    .map_err(|err| err.to_string())?;
  file.write_all(HEADER).map_err(|err| err.to_string())?;
  for hit in hits {
    write_off(&mut file, hit)?;
  }
  file.sync_all().map_err(|err| err.to_string())
}
