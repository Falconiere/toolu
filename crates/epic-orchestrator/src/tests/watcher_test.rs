use std::process::Command;
use std::thread;
use std::time::Duration;

use crate::PROTOCOL;
use crate::checkpoint::snapshot;
use crate::client::exchange_retry;
use crate::paths::Paths;
use crate::server::{Engine, Fault};
use crate::socket::serve;

fn git(cwd: &std::path::Path, args: &[&str]) {
  let status = Command::new("git")
    .args(args)
    .current_dir(cwd)
    .status()
    .expect("git");
  assert!(status.success(), "{args:?}");
}

fn counts(trace: &std::path::Path) -> (usize, usize, usize, usize, usize) {
  let text = std::fs::read_to_string(trace).unwrap_or_default();
  let start = |needle: &str| {
    text
      .lines()
      .filter(|line| line.contains("\"event\":\"start\"") && line.contains(needle))
      .count()
  };
  (
    start("read-tree"),
    start("\"add\",\"-A\""),
    start("write-tree"),
    start("commit-tree"),
    start("update-ref"),
  )
}

#[test]
fn watcher_engine() {
  let tmp = tempfile::tempdir().expect("temp");
  let work = tmp.path().join("work");
  std::fs::create_dir_all(&work).expect("work");
  git(&work, &["init"]);
  git(
    &work,
    &["config", "user.email", "epic-orchestrator@localhost"],
  );
  git(&work, &["config", "user.name", "epic-orchestrator"]);
  std::fs::write(work.join("file"), "one\n").expect("file");
  git(&work, &["add", "file"]);
  git(&work, &["commit", "-m", "base"]);
  std::fs::write(work.join("file"), "dirty\n").expect("dirty");
  let trace = tmp.path().join("trace");
  snapshot(&work.display().to_string(), "a", Some(&trace)).expect("snapshot");
  let got = counts(&trace);
  assert_eq!(
    got,
    (1, 1, 1, 1, 1),
    "{}",
    std::fs::read_to_string(&trace).unwrap_or_default()
  );
  let before = std::fs::metadata(&trace).expect("trace").len();
  let _loaded = Engine::open(Paths::at(tmp.path()), None, Fault::None).expect("reload");
  assert_eq!(std::fs::metadata(&trace).expect("trace").len(), before);

  assert_busy(&tmp.path().join("engine"));
  assert_backoff(&tmp.path().join("watch"));
}

fn assert_busy(root: &std::path::Path) {
  let paths = Paths::at(root);
  std::fs::create_dir_all(&paths.root).expect("root");
  let background = paths.clone();
  let handle = thread::spawn(move || serve(background, None, Fault::None, PROTOCOL, &()));
  let lock = paths.lock();
  for _ in 0..50 {
    if crate::lock::live(&lock) {
      break;
    }
    thread::sleep(Duration::from_millis(20));
  }
  assert!(crate::lock::live(&lock), "engine did not lock");
  let err = serve(paths.clone(), None, Fault::None, PROTOCOL, &()).expect_err("second");
  assert!(err.contains("engine-busy"), "{err}");
  let _stopped = exchange_retry(&paths, PROTOCOL, &serde_json::json!({"op": "stop"}));
  handle.join().expect("engine").expect("serve");
}

fn assert_backoff(root: &std::path::Path) {
  let watched = Paths::at(root);
  std::fs::create_dir_all(&watched.root).expect("watch");
  let mut engine = Engine::open(watched.clone(), None, Fault::None).expect("watch open");
  engine.herdr_session = Some("toolu-epic-engine-absent".to_owned());
  engine.probe_herdr().expect("probe");
  let failures = engine.world.herdr_failures;
  let retry = engine.world.herdr_retry_at_ms;
  assert!(failures >= 1);
  drop(engine);
  let mut again = Engine::open(watched, None, Fault::None).expect("again");
  again.herdr_session = Some("toolu-epic-engine-absent".to_owned());
  again.probe_herdr().expect("backoff");
  assert_eq!(again.world.herdr_failures, failures);
  assert_eq!(again.world.herdr_retry_at_ms, retry);
}
