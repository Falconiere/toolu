//! Read the retained tail after a client sequence.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use super::{Off, Summary, note, offsets_path, parsed};

/// Records strictly after `since`, at most `limit`, without reading earlier lines.
pub(super) fn collect(
  dir: &Path,
  summary: &Summary,
  since: u64,
  limit: usize,
) -> Result<Vec<super::super::Record>, String> {
  if since >= summary.last || limit == 0 {
    return Ok(Vec::new());
  }
  let mut file = File::open(offsets_path(dir)).map_err(|err| err.to_string())?;
  let mut index = first_after(&mut file, summary.records, since)?;
  let mut out = Vec::new();
  while index < summary.records && out.len() < limit {
    let hit = read_off(&mut file, index)?;
    index = index.saturating_add(1);
    if let Some(row) = read_record(dir, summary, &hit)? {
      out.push(row);
    }
  }
  Ok(out)
}

fn first_after(file: &mut File, records: u64, since: u64) -> Result<u64, String> {
  let mut lo = 0u64;
  let mut hi = records;
  while lo < hi {
    let mid = lo + (hi - lo) / 2;
    if read_off(file, mid)?.seq > since {
      hi = mid;
    } else {
      lo = mid.saturating_add(1);
    }
  }
  Ok(lo)
}

fn read_off(file: &mut File, index: u64) -> Result<Off, String> {
  let Some(at) = index
    .checked_mul(super::ROW)
    .and_then(|row| row.checked_add(5))
  else {
    return Err("corrupt journal index".to_owned());
  };
  file
    .seek(SeekFrom::Start(at))
    .map_err(|err| err.to_string())?;
  let mut buf = [0u8; 41];
  file
    .read_exact(&mut buf)
    .map_err(|_err| "corrupt journal index".to_owned())?;
  parse_off(&buf)
}

fn parse_off(buf: &[u8]) -> Result<Off, String> {
  if buf.len() != 41 || buf.last() != Some(&b'\n') {
    return Err("corrupt journal index".to_owned());
  }
  let seq = parse_hex(field(buf, 0, 16)?)?;
  let file = parse_hex(field(buf, 16, 24)?)?;
  let pos = parse_hex(field(buf, 24, 40)?)?;
  Ok(Off {
    seq,
    file: u32::try_from(file).map_err(|err| err.to_string())?,
    pos,
  })
}

fn field(buf: &[u8], from: usize, to: usize) -> Result<&[u8], String> {
  buf
    .get(from..to)
    .ok_or_else(|| "corrupt journal index".to_owned())
}

fn parse_hex(bytes: &[u8]) -> Result<u64, String> {
  let text = std::str::from_utf8(bytes).map_err(|_err| "corrupt journal index".to_owned())?;
  u64::from_str_radix(text, 16).map_err(|_err| "corrupt journal index".to_owned())
}

fn read_record(
  dir: &Path,
  summary: &Summary,
  hit: &Off,
) -> Result<Option<super::super::Record>, String> {
  let index = usize::try_from(hit.file).map_err(|err| err.to_string())?;
  let day = summary
    .files
    .get(index)
    .ok_or_else(|| "corrupt journal index".to_owned())?;
  let mut file =
    File::open(dir.join(format!("{}.jsonl", day.date))).map_err(|err| err.to_string())?;
  file
    .seek(SeekFrom::Start(hit.pos))
    .map_err(|err| err.to_string())?;
  Ok(parsed(&read_line(&mut file)?))
}

#[cfg(test)]
#[path = "tests/read_test.rs"]
mod tests;

fn read_line(file: &mut File) -> Result<Vec<u8>, String> {
  let mut buf = Vec::new();
  let mut chunk = [0u8; 256];
  loop {
    let n = file.read(&mut chunk).map_err(|err| err.to_string())?;
    if n == 0 {
      return Ok(buf);
    }
    note(n);
    let Some(got) = chunk.get(..n) else {
      return Ok(buf);
    };
    if let Some(at) = got.iter().position(|byte| *byte == b'\n') {
      buf.extend_from_slice(got.get(..at).unwrap_or_default());
      return Ok(buf);
    }
    buf.extend_from_slice(got);
  }
}
