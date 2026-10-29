use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::{Duration, SystemTime};

use toolu_runtime::env::Env;

use super::{
  BYTE_SAVINGS_MAX_COMPLETED, BYTE_SAVINGS_RETENTION, active_session_from_payload,
  sweep_byte_savings, sweep_dir,
};
use crate::report::{parse, read_ledger, render};
use crate::savings::byte_savings_dir;

const DAY: Duration = Duration::from_hours(24);
const LINE: &str = "{\"kind\":\"read\",\"returned\":10,\"full\":100}\n";

fn put(path: &Path, body: &str, age: Duration) {
  if let Some(parent) = path.parent() {
    fs::create_dir_all(parent).expect("parent");
  }
  fs::write(path, body).expect("write");
  let file = File::options().write(true).open(path).expect("open");
  file.set_modified(SystemTime::now() - age).expect("mtime");
}

fn ledger(dir: &Path, stem: &str) -> PathBuf {
  dir.join(format!("{stem}.jsonl"))
}

fn count_jsonl(dir: &Path) -> usize {
  fs::read_dir(dir).map_or(0, |entries| {
    entries
      .filter_map(Result::ok)
      .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "jsonl"))
      .count()
  })
}

#[test]
fn sweep_deletes_cold_keeps_active_and_caps_completed() {
  let root = tempfile::tempdir().expect("config");
  let dir = root.path().join("toolu/byte-savings");
  fs::create_dir_all(&dir).expect("dir");
  for i in 0..70 {
    put(
      &ledger(&dir, &format!("cold-{i}")),
      LINE,
      BYTE_SAVINGS_RETENTION + DAY,
    );
  }
  for i in 0..10 {
    put(
      &ledger(&dir, &format!("hot-{i}")),
      LINE,
      Duration::from_secs(60),
    );
  }
  put(
    &ledger(&dir, "active-session"),
    LINE,
    BYTE_SAVINGS_RETENTION + DAY * 30,
  );
  let before = count_jsonl(&dir);
  assert_eq!(before, 81);
  let warnings = sweep_dir(&dir, Some("active-session"), SystemTime::now());
  assert_eq!(warnings, Vec::<String>::new());
  assert!(ledger(&dir, "active-session").is_file());
  for i in 0..70 {
    assert!(!ledger(&dir, &format!("cold-{i}")).exists());
  }
  assert_eq!(count_jsonl(&dir), 11); // 10 hot + active
  assert!(count_jsonl(&dir) - 1 <= BYTE_SAVINGS_MAX_COMPLETED);
}

#[test]
fn sweep_enforces_max_completed_among_hot_non_active() {
  let root = tempfile::tempdir().expect("config");
  let dir = root.path().join("toolu/byte-savings");
  fs::create_dir_all(&dir).expect("dir");
  let total = BYTE_SAVINGS_MAX_COMPLETED + 5;
  for i in 0..total {
    // Distinct ages so oldest-first is deterministic.
    put(
      &ledger(&dir, &format!("hot-{i}")),
      LINE,
      Duration::from_secs(60 + i as u64),
    );
  }
  put(&ledger(&dir, "live"), LINE, Duration::from_secs(1));
  let warnings = sweep_dir(&dir, Some("live"), SystemTime::now());
  assert_eq!(warnings, Vec::<String>::new());
  assert!(ledger(&dir, "live").is_file());
  assert_eq!(count_jsonl(&dir), BYTE_SAVINGS_MAX_COMPLETED + 1);
  // Oldest five hot-* must be gone (highest age offset).
  for i in (total - 5)..total {
    assert!(
      !ledger(&dir, &format!("hot-{i}")).exists(),
      "expected hot-{i} deleted"
    );
  }
}

#[test]
fn retained_ledger_bytes_and_savings_report_unchanged() {
  let root = tempfile::tempdir().expect("config");
  let dir = root.path().join("toolu/byte-savings");
  let retained = ledger(&dir, "keep-me");
  let mut body = String::new();
  for _ in 0..5 {
    body.push_str(LINE);
  }
  put(&retained, &body, Duration::from_secs(30));
  put(&ledger(&dir, "cold"), LINE, BYTE_SAVINGS_RETENTION + DAY);
  let before = fs::read(&retained).expect("read");
  let report_before = render(&parse(&read_ledger(&retained).expect("ledger")).expect("parse"));
  assert_eq!(
    sweep_dir(&dir, Some("keep-me"), SystemTime::now()),
    Vec::<String>::new()
  );
  assert_eq!(fs::read(&retained).expect("read"), before);
  let report_after = render(&parse(&read_ledger(&retained).expect("ledger")).expect("parse"));
  assert_eq!(report_before, report_after);
  assert!(!ledger(&dir, "cold").exists());
}

#[test]
fn concurrent_append_on_active_ledger_loses_no_line() {
  let root = tempfile::tempdir().expect("config");
  let dir = root.path().join("toolu/byte-savings");
  fs::create_dir_all(&dir).expect("dir");
  let active = Arc::new(ledger(&dir, "ses-live"));
  put(&active, "", Duration::from_secs(0));
  let barrier = Arc::new(Barrier::new(2));
  let writer = {
    let active = Arc::clone(&active);
    let barrier = Arc::clone(&barrier);
    thread::spawn(move || {
      barrier.wait();
      let mut file = File::options().append(true).open(&*active).expect("append");
      for _ in 0..200 {
        file.write_all(LINE.as_bytes()).expect("write");
      }
    })
  };
  let sweeper = {
    let barrier = Arc::clone(&barrier);
    thread::spawn(move || {
      barrier.wait();
      for _ in 0..50 {
        let _ = sweep_dir(&dir, Some("ses-live"), SystemTime::now());
      }
    })
  };
  writer.join().expect("writer");
  sweeper.join().expect("sweeper");
  assert!(active.is_file());
  let text = fs::read_to_string(&*active).expect("read");
  assert_eq!(text.lines().filter(|line| !line.is_empty()).count(), 200);
  parse(&text).expect("every line parses");
}

#[test]
fn missing_directory_is_noop_and_roots_stay_isolated() {
  let missing = tempfile::tempdir().expect("missing root");
  let env = Env::from_pairs([("TOOLU_CONFIG_DIR", missing.path().display().to_string())]);
  assert_eq!(sweep_byte_savings(&env, Some("s")), Vec::<String>::new());
  assert!(!byte_savings_dir(&env).exists());

  let other = tempfile::tempdir().expect("other");
  let other_dir = other.path().join("toolu/byte-savings");
  put(
    &ledger(&other_dir, "foreign"),
    LINE,
    BYTE_SAVINGS_RETENTION + DAY,
  );
  let mine = tempfile::tempdir().expect("mine");
  let mine_dir = mine.path().join("toolu/byte-savings");
  put(
    &ledger(&mine_dir, "cold"),
    LINE,
    BYTE_SAVINGS_RETENTION + DAY,
  );
  let env = Env::from_pairs([("TOOLU_CONFIG_DIR", mine.path().display().to_string())]);
  assert_eq!(sweep_byte_savings(&env, None), Vec::<String>::new());
  assert!(!ledger(&mine_dir, "cold").exists());
  assert!(ledger(&other_dir, "foreign").is_file());
}

#[test]
fn retention_churn_bounds_completed_ledgers() {
  let root = tempfile::tempdir().expect("config");
  let dir = root.path().join("toolu/byte-savings");
  fs::create_dir_all(&dir).expect("dir");
  for i in 0..70 {
    put(
      &ledger(&dir, &format!("cold-{i}")),
      &format!("{{\"kind\":\"read\",\"returned\":{i},\"full\":100}}\n"),
      BYTE_SAVINGS_RETENTION + DAY,
    );
  }
  for i in 0..10 {
    put(
      &ledger(&dir, &format!("hot-{i}")),
      LINE,
      Duration::from_secs(30),
    );
  }
  put(
    &ledger(&dir, "active"),
    LINE,
    BYTE_SAVINGS_RETENTION + DAY * 40,
  );
  let before_count = count_jsonl(&dir);
  let before_bytes: u64 = fs::read_dir(&dir)
    .expect("list")
    .filter_map(Result::ok)
    .filter_map(|entry| fs::metadata(entry.path()).ok())
    .map(|meta| meta.len())
    .sum();
  let warnings = sweep_dir(&dir, Some("active"), SystemTime::now());
  let after_count = count_jsonl(&dir);
  let after_bytes: u64 = fs::read_dir(&dir)
    .expect("list")
    .filter_map(Result::ok)
    .filter_map(|entry| fs::metadata(entry.path()).ok())
    .map(|meta| meta.len())
    .sum();
  println!(
    "byte-savings retention churn: files {before_count} -> {after_count}; bytes {before_bytes} -> {after_bytes}; warnings={warnings:?}"
  );
  assert_eq!(before_count, 81);
  assert_eq!(after_count, 11);
  assert!(after_bytes < before_bytes);
  assert!(ledger(&dir, "active").is_file());
}

#[test]
fn payload_session_id_matches_savings_sanitization() {
  assert_eq!(
    active_session_from_payload(r#"{"session_id":"ses_real-1"}"#).as_deref(),
    Some("sesreal-1")
  );
  assert_eq!(active_session_from_payload("{}"), None);
  assert_eq!(active_session_from_payload(r#"{"session_id":""}"#), None);
}
