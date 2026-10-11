use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, UNIX_EPOCH};

use toolu_runtime::env::Env;
use toolu_runtime::git::{Discovery, discover};
use toolu_runtime::host::roots::Roots;

use super::{acquire, fresh_at, millis_since_epoch, paths, read, read_record, refresh, release};

#[test]
fn invalid_clock_and_zero_timestamp_never_make_a_cache_record_fresh() {
  let before_epoch = UNIX_EPOCH - Duration::from_millis(1);
  let invalid_now = millis_since_epoch(before_epoch);
  assert_eq!(invalid_now, 0);
  assert!(!fresh_at(0, invalid_now));
  assert!(!fresh_at(0, 1_000));
  assert!(!fresh_at(1_000, invalid_now));
  assert!(fresh_at(1_000, 3_000));
  assert!(!fresh_at(1_000, 3_001));
}

fn git(dir: &Path, args: &[&str]) {
  let result = Command::new("git")
    .arg("-C")
    .arg(dir)
    .args(args)
    .output()
    .unwrap();
  assert!(
    result.status.success(),
    "{}",
    String::from_utf8_lossy(&result.stderr)
  );
}

#[test]
fn real_git_refresh_writes_one_atomic_record_and_obeys_the_lock() {
  let temp = tempfile::tempdir().unwrap();
  let root = temp.path().join("repo");
  fs::create_dir_all(&root).unwrap();
  git(&root, &["init", "-q", "-b", "main"]);
  git(
    &root,
    &[
      "-c",
      "user.email=t@t",
      "-c",
      "user.name=t",
      "commit",
      "-q",
      "--allow-empty",
      "-m",
      "init",
    ],
  );
  fs::write(root.join("new.txt"), "real file\n").unwrap();
  let env = Env::from_pairs([
    ("HOME", temp.path().display().to_string()),
    (
      "CLAUDE_CONFIG_DIR",
      temp.path().join("cfg").display().to_string(),
    ),
    ("PATH", std::env::var("PATH").unwrap()),
  ]);
  let roots = Roots::new(env.clone(), Some(toolu_protocol::host::Host::Claude));
  let Discovery::Repo(repo) = discover(&env, &root) else {
    panic!("real repo");
  };
  let paths = paths(&roots, &repo).unwrap();
  let token = acquire(&paths.lock).unwrap();
  refresh(&roots, &repo).unwrap();
  assert!(!paths.cache.exists());
  release(&paths.lock, &token);
  refresh(&roots, &repo).unwrap();
  assert_eq!(read(&roots, &repo).untracked, 1);
  assert_eq!(read_record(&paths.cache).unwrap().counts.untracked, 1);
  assert!(!paths.lock.exists());
  assert_eq!(
    fs::read_dir(paths.cache.parent().unwrap()).unwrap().count(),
    1
  );
}
