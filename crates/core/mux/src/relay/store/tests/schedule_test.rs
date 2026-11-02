use super::*;
use crate::relay::store::tests::{config, submission};

#[test]
fn reservation_snapshots_recipes_and_worker_settings() {
  let dir = tempfile::tempdir().unwrap();
  let store = Store::new(dir.path());
  let mut cfg = config(&[("worker", 1, false, &["codex"])]);
  cfg
    .recipes
    .insert("origin".into(), vec![vec!["cargo".into(), "fetch".into()]]);
  store.submit(submission("one"), b"input").unwrap();
  let task = store.reserve("one", &cfg, 0).unwrap().unwrap();
  cfg.recipes.clear();
  cfg.machines.clear();
  assert_eq!(task.recipes, vec![vec!["cargo", "fetch"]]);
  assert_eq!(
    store.load("one").unwrap().worker.unwrap().root,
    "/tmp/worker"
  );
}

#[test]
fn failed_worker_cools_across_restart_without_starving_other_workers() {
  let dir = tempfile::tempdir().unwrap();
  let store = Store::new(dir.path());
  let cfg = config(&[
    ("alpha", 1, false, &["codex"]),
    ("beta", 1, false, &["codex"]),
  ]);
  store.submit(submission("one"), b"input").unwrap();
  let first = store.reserve("one", &cfg, 100).unwrap().unwrap();
  assert_eq!(first.machine.as_deref(), Some("alpha"));
  let attempt = first.attempt.as_deref().unwrap();
  let retry = store.retry("one", attempt, 100, "unreachable").unwrap();
  assert!(store.retry("one", "stale", 10_000, "stale").is_err());
  let reopened = Store::new(dir.path());
  let second = reopened
    .reserve("one", &cfg, retry.next_at)
    .unwrap()
    .unwrap();
  assert_eq!(second.machine.as_deref(), Some("beta"));
  let mut pinned = submission("pinned");
  pinned.machine = Some("alpha".into());
  reopened.submit(pinned, b"input").unwrap();
  assert!(
    reopened
      .reserve("pinned", &cfg, retry.next_at)
      .unwrap()
      .is_none()
  );
  let until = retry.next_at + (retry.next_at - 100);
  assert_eq!(
    reopened
      .reserve("pinned", &cfg, until)
      .unwrap()
      .unwrap()
      .machine
      .as_deref(),
    Some("alpha")
  );
}
