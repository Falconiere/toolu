use std::fs::File;
use std::path::Path;
use std::time::{Duration, Instant, SystemTime};

use super::{LockOptions, lock_path, token, with_lock};

fn age(lock: &Path, by: Duration) {
  let file = File::options().write(true).open(lock).unwrap();
  file.set_modified(SystemTime::now() - by).unwrap();
}

fn leftovers(dir: &Path) -> Vec<String> {
  std::fs::read_dir(dir)
    .unwrap()
    .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
    .filter(|name| name.contains(".lock"))
    .collect()
}

/// A pid that existed and is gone: a reaped child's.
fn dead_pid() -> u32 {
  let mut child = std::process::Command::new("true").spawn().unwrap();
  let pid = child.id();
  child.wait().unwrap();
  pid
}

#[test]
fn the_result_is_returned_and_the_lock_released_even_on_panic() {
  let dir = tempfile::tempdir().unwrap();
  let file = dir.path().join("gate.json");
  let mut warnings = Vec::new();
  let seen = with_lock(&file, LockOptions::default(), &mut warnings, || {
    std::fs::read_to_string(lock_path(&file)).unwrap()
  });
  let (pid, uuid) = seen.trim_end().split_once(' ').unwrap();
  assert_eq!(pid, std::process::id().to_string());
  assert_eq!(uuid.len(), 36);
  assert!(seen.ends_with('\n'));
  assert!(!lock_path(&file).exists());
  let panicked = std::panic::catch_unwind(|| {
    with_lock(&file, LockOptions::default(), &mut Vec::new(), || {
      panic!("boom")
    })
  });
  assert!(panicked.is_err());
  assert!(!lock_path(&file).exists(), "released on unwind");
  assert_eq!(warnings, Vec::<String>::new());
}

#[test]
fn a_stale_lock_from_a_crashed_writer_is_broken_without_litter() {
  let dir = tempfile::tempdir().unwrap();
  let file = dir.path().join("gate.json");
  std::fs::write(lock_path(&file), "99999 crashed\n").unwrap();
  age(&lock_path(&file), Duration::from_secs(60));
  let mut warnings = Vec::new();
  assert_eq!(
    with_lock(&file, LockOptions::default(), &mut warnings, || 7),
    7
  );
  assert_eq!(warnings, Vec::<String>::new());
  assert_eq!(leftovers(dir.path()), Vec::<String>::new());
}

#[test]
fn a_fresh_lock_whose_holder_is_gone_is_broken_at_once() {
  let dir = tempfile::tempdir().unwrap();
  let file = dir.path().join("gate.json");
  std::fs::write(lock_path(&file), format!("{} crashed-token\n", dead_pid())).unwrap();
  let started = Instant::now();
  with_lock(&file, LockOptions::default(), &mut Vec::new(), || ());
  assert!(started.elapsed() < Duration::from_secs(1));
  assert!(!lock_path(&file).exists());
}

#[test]
fn a_live_holders_lock_is_broken_once_older_than_the_stale_age() {
  let dir = tempfile::tempdir().unwrap();
  let file = dir.path().join("gate.json");
  std::fs::write(
    lock_path(&file),
    format!("{} hung-token\n", std::process::id()),
  )
  .unwrap();
  age(&lock_path(&file), Duration::from_secs(3));
  let mut warnings = Vec::new();
  with_lock(&file, LockOptions::default(), &mut warnings, || ());
  assert_eq!(warnings, Vec::<String>::new());
  assert!(!lock_path(&file).exists());
}

#[test]
fn a_live_fresh_lock_times_out_warns_and_runs_unlocked() {
  let dir = tempfile::tempdir().unwrap();
  let file = dir.path().join("gate.json");
  let theirs = format!("{} live\n", std::process::id());
  std::fs::write(lock_path(&file), &theirs).unwrap();
  let options = LockOptions {
    timeout: Duration::from_millis(100),
    ..LockOptions::default()
  };
  let mut warnings = Vec::new();
  assert!(with_lock(&file, options, &mut warnings, || true));
  let lock = lock_path(&file).display().to_string();
  assert_eq!(
    warnings,
    [format!("state: lock {lock} still held; writing without it")]
  );
  assert_eq!(std::fs::read_to_string(lock_path(&file)).unwrap(), theirs);
}

#[test]
fn a_lock_another_holder_took_over_is_never_released() {
  let dir = tempfile::tempdir().unwrap();
  let file = dir.path().join("gate.json");
  with_lock(&file, LockOptions::default(), &mut Vec::new(), || {
    std::fs::write(lock_path(&file), "4242 their-token\n").unwrap();
  });
  assert_eq!(
    std::fs::read_to_string(lock_path(&file)).unwrap(),
    "4242 their-token\n"
  );
}

#[test]
fn a_lock_that_cannot_be_created_runs_unlocked_silently() {
  let dir = tempfile::tempdir().unwrap();
  let file = dir.path().join("missing").join("gate.json");
  let mut warnings = Vec::new();
  assert_eq!(
    with_lock(&file, LockOptions::default(), &mut warnings, || 1),
    1
  );
  assert_eq!(warnings, Vec::<String>::new());
}

#[test]
fn tokens_are_version_4_uuids_and_differ() {
  let (a, b) = (token(), token());
  assert_ne!(a, b);
  let parts: Vec<&str> = a.split('-').collect();
  assert_eq!(
    parts.iter().map(|part| part.len()).collect::<Vec<_>>(),
    [8, 4, 4, 4, 12]
  );
  assert!(parts[2].starts_with('4'));
  assert!(matches!(
    parts[3].chars().next(),
    Some('8' | '9' | 'a' | 'b')
  ));
}
