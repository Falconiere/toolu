use std::io::Write;
use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, UNIX_EPOCH};

use super::super::{Record, append, file_for, page};
use super::SINK;

fn plant(dir: &Path, lock: &Path, days: u64) -> Vec<u64> {
  let start = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
  (0..days)
    .map(|day| {
      let now = start + Duration::from_secs(day * 86_400);
      append(
        dir,
        lock,
        &Record::new("action", "merge-intent", "k", "t", "n"),
        now,
      )
      .unwrap()
      .seq
    })
    .collect()
}

fn oracle(dir: &Path) -> Vec<u64> {
  let mut seqs = Vec::new();
  for entry in std::fs::read_dir(dir).unwrap() {
    let path = entry.unwrap().path();
    if path.extension().is_none_or(|ext| ext != "jsonl") {
      continue;
    }
    for line in std::fs::read_to_string(&path).unwrap().lines() {
      let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
        continue;
      };
      if let Some(seq) = value.get("seq").and_then(serde_json::Value::as_u64) {
        seqs.push(seq);
      }
    }
  }
  seqs.sort_unstable();
  seqs
}

fn notes(dir: &Path) -> Vec<String> {
  let mut out = Vec::new();
  for entry in std::fs::read_dir(dir).unwrap() {
    let path = entry.unwrap().path();
    if path.extension().is_none_or(|ext| ext != "jsonl") {
      continue;
    }
    for line in std::fs::read_to_string(&path).unwrap().lines() {
      let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
        continue;
      };
      if let Some(note) = value.get("note").and_then(serde_json::Value::as_str) {
        out.push(note.to_owned());
      }
    }
  }
  out
}

fn jsonl_bytes(dir: &Path) -> u64 {
  std::fs::read_dir(dir)
    .unwrap()
    .map(|entry| {
      let path = entry.unwrap().path();
      if path.extension().is_some_and(|ext| ext == "jsonl") {
        std::fs::metadata(path).unwrap().len()
      } else {
        0
      }
    })
    .sum()
}

fn spawn(dir: &Path, lock: &Path, note: &str, count: &str, secs: &str) -> std::process::Child {
  Command::new(std::env::current_exe().unwrap())
    .arg("--exact")
    .arg("journal::index::tests::external_journal_writer")
    .env("TOOLU_JOURNAL_DIR", dir)
    .env("TOOLU_JOURNAL_LOCK", lock)
    .env("TOOLU_JOURNAL_NOTE", note)
    .env("TOOLU_JOURNAL_COUNT", count)
    .env("TOOLU_JOURNAL_SECS", secs)
    .stdout(Stdio::null())
    .stderr(Stdio::piped())
    .spawn()
    .unwrap()
}

#[test]
fn external_journal_writer() {
  let Ok(dir) = std::env::var("TOOLU_JOURNAL_DIR") else {
    return;
  };
  let lock = std::env::var("TOOLU_JOURNAL_LOCK").unwrap();
  let note = std::env::var("TOOLU_JOURNAL_NOTE").unwrap();
  let count: u32 = std::env::var("TOOLU_JOURNAL_COUNT")
    .unwrap()
    .parse()
    .unwrap();
  let secs: u64 = std::env::var("TOOLU_JOURNAL_SECS")
    .unwrap()
    .parse()
    .unwrap();
  let now = UNIX_EPOCH + Duration::from_secs(secs);
  for _ in 0..count {
    append(
      Path::new(&dir),
      Path::new(&lock),
      &Record::new("action", "merge-intent", "k", "t", &note),
      now,
    )
    .unwrap();
  }
}

#[test]
fn missing_journal_directory_pages_empty() {
  let dir = tempfile::tempdir().unwrap();
  let lock = dir.path().join("journal.lock");
  let page = page(&dir.path().join("absent"), &lock, 0, 1).unwrap();
  assert_eq!((page.0, page.1.len()), (None, 0));
}

#[test]
fn two_writers_restart_and_rollover_do_not_reuse_sequences() {
  let dir = tempfile::tempdir().unwrap();
  let journal = dir.path().join("journal");
  let lock = dir.path().join("journal.lock");
  std::fs::create_dir_all(&journal).unwrap();
  let day = "1700000000";
  let left = spawn(&journal, &lock, "left", "8", day);
  let right = spawn(&journal, &lock, "right", "8", day);
  let left = left.wait_with_output().unwrap();
  let right = right.wait_with_output().unwrap();
  assert!(
    left.status.success(),
    "{}",
    String::from_utf8_lossy(&left.stderr)
  );
  assert!(
    right.status.success(),
    "{}",
    String::from_utf8_lossy(&right.stderr)
  );
  let later = UNIX_EPOCH + Duration::from_secs(1_700_000_000 + 72 * 3_600);
  let stored = append(
    &journal,
    &lock,
    &Record::new("action", "merge-intent", "k", "t", "parent"),
    later,
  )
  .unwrap();
  let seqs = oracle(&journal);
  assert_eq!(seqs.len(), 17);
  assert!(seqs.windows(2).all(|pair| pair[0] < pair[1]));
  assert_eq!(seqs.last().copied(), Some(stored.seq));
  let found = notes(&journal);
  assert_eq!(found.iter().filter(|note| *note == "left").count(), 8);
  assert_eq!(found.iter().filter(|note| *note == "right").count(), 8);
}

#[test]
fn synced_line_is_recovered_and_a_torn_tail_is_not_committed() {
  let dir = tempfile::tempdir().unwrap();
  let journal = dir.path().join("journal");
  let lock = dir.path().join("journal.lock");
  let now = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
  append(
    &journal,
    &lock,
    &Record::new("action", "merge-intent", "k", "t", "n"),
    now,
  )
  .unwrap();
  let path = file_for(&journal, now);
  let mut file = std::fs::OpenOptions::new()
    .append(true)
    .open(&path)
    .unwrap();
  writeln!(file, r#"{{"seq":2,"name":"planted"}}"#).unwrap();
  file.sync_all().unwrap();
  let rows = page(&journal, &lock, 0, 10).unwrap().1;
  assert_eq!(rows.iter().map(|row| row.seq).collect::<Vec<_>>(), [1, 2]);
  write!(file, r#"{{"seq":99"#).unwrap();
  file.sync_all().unwrap();
  let rows = page(&journal, &lock, 0, 10).unwrap().1;
  assert!(rows.iter().all(|row| row.seq != 99));
  std::fs::remove_file(journal.join("sequence.index")).unwrap();
  std::fs::remove_file(journal.join("sequence.offsets")).unwrap();
  let stored = append(
    &journal,
    &lock,
    &Record::new("action", "merge-done", "k", "t", "n"),
    now,
  )
  .unwrap();
  assert_eq!(stored.seq, 3);
  let rows = page(&journal, &lock, 0, 10).unwrap().1;
  assert!(rows.iter().any(|row| row.seq == 3));
  assert!(rows.iter().all(|row| row.seq != 99));
}

#[test]
fn unreadable_journal_file_returns_an_error() {
  let dir = tempfile::tempdir().unwrap();
  let journal = dir.path().join("journal");
  let lock = dir.path().join("journal.lock");
  let now = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
  append(
    &journal,
    &lock,
    &Record::new("action", "merge-intent", "k", "t", "n"),
    now,
  )
  .unwrap();
  let path = file_for(&journal, now);
  std::fs::remove_file(journal.join("sequence.index")).unwrap();
  let mut perms = std::fs::metadata(&path).unwrap().permissions();
  perms.set_mode(0o000);
  std::fs::set_permissions(&path, perms.clone()).unwrap();
  let err = page(&journal, &lock, 0, 10).unwrap_err();
  perms.set_mode(0o644);
  std::fs::set_permissions(&path, perms).unwrap();
  assert!(!err.is_empty(), "{err}");
}

#[test]
fn indexed_page_matches_a_full_scan_and_bounds_reads() {
  let dir = tempfile::tempdir().unwrap();
  let journal = dir.path().join("journal");
  let lock = dir.path().join("journal.lock");
  let seqs = plant(&journal, &lock, 90);
  let scanned = oracle(&journal);
  assert_eq!(seqs, scanned);
  check_cursors(&journal, &lock, &seqs, &scanned);
  let sink = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
  SINK.with(|slot| *slot.borrow_mut() = Some(std::sync::Arc::clone(&sink)));
  let _disarm = Disarm;
  let started = Instant::now();
  for _ in 0..20 {
    assert_eq!(page(&journal, &lock, seqs[89], 10).unwrap().1.len(), 0);
  }
  let idle = sink.swap(0, std::sync::atomic::Ordering::Relaxed);
  let idle_ms = started.elapsed().as_millis();
  let started = Instant::now();
  let now = UNIX_EPOCH + Duration::from_secs(1_700_000_000 + 89 * 86_400);
  for _ in 0..20 {
    append(
      &journal,
      &lock,
      &Record::new("action", "merge-intent", "k", "t", "n"),
      now,
    )
    .unwrap();
  }
  let append_bytes = sink.swap(0, std::sync::atomic::Ordering::Relaxed);
  let journal_bytes = jsonl_bytes(&journal);
  let line = format!(
    "platform={} journal_bytes={journal_bytes} idle_jsonl={idle} idle_ms={idle_ms} append_jsonl={append_bytes} append_ms={} samples=20\n",
    std::env::consts::OS,
    started.elapsed().as_millis()
  );
  std::fs::write("/tmp/toolu-757-measure.txt", line).unwrap();
  assert_eq!((idle, append_bytes, journal_bytes > 0), (0, 0, true));
}

struct Disarm;

impl Drop for Disarm {
  fn drop(&mut self) {
    SINK.with(|slot| *slot.borrow_mut() = None);
  }
}

fn check_cursors(dir: &Path, lock: &Path, seqs: &[u64], scanned: &[u64]) {
  let oldest = page(dir, lock, 0, 1).unwrap().0;
  assert_eq!(oldest, Some(seqs[0]));
  for (since, limit) in [(0, 10), (seqs[40], 1), (seqs[89], 10), (seqs[89] + 1, 10)] {
    let got: Vec<u64> = page(dir, lock, since, limit)
      .unwrap()
      .1
      .iter()
      .map(|row| row.seq)
      .collect();
    let expect: Vec<u64> = scanned
      .iter()
      .copied()
      .filter(|seq| *seq > since)
      .take(limit)
      .collect();
    assert_eq!(got, expect, "since {since} limit {limit}");
  }
}
