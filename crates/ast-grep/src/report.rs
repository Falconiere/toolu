//! The JSONL byte-savings ledger and the human report format.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

use serde::Deserialize;

/// Maximum byte size accepted from a savings ledger.
pub(crate) const MAX_LEDGER_BYTES: u64 = 16 * 1024 * 1024;

/// Read a ledger without allowing a file to consume unbounded memory.
pub(crate) fn read_ledger(path: &Path) -> io::Result<String> {
  let file = File::open(path)?;
  let mut bytes = Vec::new();
  file.take(MAX_LEDGER_BYTES + 1).read_to_end(&mut bytes)?;
  if bytes.len() as u64 > MAX_LEDGER_BYTES {
    return Err(io::Error::new(
      io::ErrorKind::InvalidData,
      format!("ledger exceeds {MAX_LEDGER_BYTES} bytes"),
    ));
  }
  String::from_utf8(bytes).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

/// One recorded tool response's byte counts.
#[derive(Clone, Debug, Deserialize)]
pub(crate) struct Record {
  /// Tool category.
  pub(crate) kind: String,
  /// UTF-8 bytes returned into context.
  pub(crate) returned: f64,
  /// The complete file's byte size for Read, zero otherwise.
  pub(crate) full: f64,
}

/// Parse every nonblank JSONL line, or return its first invalid 1-based line.
pub(crate) fn parse(text: &str) -> Result<Vec<Record>, usize> {
  text
    .lines()
    .enumerate()
    .filter(|(_, line)| !line.trim().is_empty())
    .map(|(at, line)| serde_json::from_str(line).map_err(|_error| at + 1))
    .collect()
}

/// Sum counts by kind and append the total and approximate token count.
pub(crate) fn render(records: &[Record]) -> String {
  let mut totals: BTreeMap<&str, (f64, f64, usize)> = BTreeMap::new();
  for record in records {
    let total = totals.entry(&record.kind).or_default();
    total.0 += record.returned;
    total.1 += record.full;
    total.2 += 1;
  }
  let mut lines = Vec::new();
  let mut sum = 0.0;
  for (kind, (returned, full, count)) in totals {
    sum += returned;
    if kind == "read" && full > 0.0 {
      let saved = ((full - returned) * 100.0 / full).floor();
      lines.push(format!(
        "{kind}: returned={returned} full={full} saved={saved}% (n={count})"
      ));
    } else {
      lines.push(format!("{kind}: returned={returned} (n={count})"));
    }
  }
  lines.push(format!(
    "TOTAL returned: {sum} bytes (~{} tok)",
    (sum / 4.0).floor()
  ));
  lines.join("\n")
}

#[cfg(test)]
#[path = "tests/report_test.rs"]
mod tests;
