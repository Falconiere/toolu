use std::fs::{self, File};
use std::path::Path;
use std::time::{Duration, SystemTime};

use super::sweep_outline_dir;

fn put(path: &Path, age: Duration) {
  fs::create_dir_all(path.parent().unwrap()).unwrap();
  fs::write(path, "{}").unwrap();
  File::options()
    .write(true)
    .open(path)
    .unwrap()
    .set_modified(SystemTime::now() - age)
    .unwrap();
}

#[test]
fn outline_sweep_removes_old_entries_then_oldest_over_capacity() {
  let root = tempfile::tempdir().unwrap();
  let dir = root.path().join("toolu/outline");
  let version = dir.join("v1-ast-grep-1/rust");
  put(&version.join("cold.json"), Duration::from_hours(24 * 15));
  for i in 0..5 {
    put(
      &version.join(format!("hot-{i}.json")),
      Duration::from_secs(60 + i),
    );
  }
  let warnings = sweep_outline_dir(&dir, SystemTime::now(), 3);
  assert!(warnings.is_empty(), "{warnings:?}");
  assert!(!version.join("cold.json").exists());
  assert!(!version.join("hot-4.json").exists());
  assert!(!version.join("hot-3.json").exists());
  for i in 0..3 {
    assert!(version.join(format!("hot-{i}.json")).exists());
  }
}
