use super::*;
use crate::relay::config::{Config, Machine};
use std::sync::{Arc, Barrier};

pub(super) fn submission(key: &str) -> Submission {
  Submission {
    key: key.into(),
    origin: "origin".into(),
    base: "a".repeat(40),
    brief: "do the work".into(),
    agent: "codex".into(),
    argv: vec!["--quiet".into()],
    machine: None,
    group: None,
  }
}

pub(super) fn config(machines: &[(&str, u32, bool, &[&str])]) -> Config {
  Config {
    version: 1,
    relay: None,
    generation: None,
    recipes: BTreeMap::default(),
    machines: machines
      .iter()
      .map(|(name, capacity, draining, agents)| {
        (
          (*name).into(),
          Machine {
            name: (*name).into(),
            root: format!("/tmp/{name}"),
            agents: agents.iter().map(|agent| (*agent).into()).collect(),
            capacity: *capacity,
            draining: *draining,
          },
        )
      })
      .collect(),
  }
}

#[test]
fn submission_replays_only_identical_payload_and_bytes() {
  let dir = tempfile::tempdir().unwrap();
  let store = Store::new(dir.path());
  let request = submission("one");
  let first = store.submit(request.clone(), b"input").unwrap();
  let replay = Store::new(dir.path())
    .submit(request.clone(), b"input")
    .unwrap();
  assert_eq!(first.request.key, replay.request.key);
  assert_eq!(first.operation, replay.operation);
  assert_eq!(first.phase, replay.phase);
  let mut changed = request.clone();
  changed.brief = "different".into();
  assert!(store.submit(changed, b"input").is_err());
  assert!(store.submit(request, b"different").is_err());
  assert_eq!(store.list().unwrap().len(), 1);
}

#[test]
fn reservation_selects_lowest_load_filters_ineligible_and_respects_pinning() {
  let dir = tempfile::tempdir().unwrap();
  let store = Store::new(dir.path());
  let cfg = config(&[
    ("alpha", 2, false, &["codex"]),
    ("beta", 1, false, &["codex"]),
    ("drain", 5, true, &["codex"]),
    ("other", 5, false, &["claude"]),
  ]);
  for key in ["one", "two", "three"] {
    store.submit(submission(key), b"input").unwrap();
  }
  assert_eq!(
    store
      .reserve("one", &cfg, 10)
      .unwrap()
      .unwrap()
      .machine
      .as_deref(),
    Some("alpha")
  );
  assert_eq!(
    store
      .reserve("two", &cfg, 10)
      .unwrap()
      .unwrap()
      .machine
      .as_deref(),
    Some("beta")
  );
  assert_eq!(
    store
      .reserve("three", &cfg, 10)
      .unwrap()
      .unwrap()
      .machine
      .as_deref(),
    Some("alpha")
  );
  let mut pinned = submission("pinned");
  pinned.machine = Some("beta".into());
  store.submit(pinned, b"input").unwrap();
  assert!(store.reserve("pinned", &cfg, 10).unwrap().is_none());
  assert_eq!(store.load("pinned").unwrap().phase, "queued");
}

#[test]
fn concurrent_reservations_never_exceed_capacity() {
  let dir = tempfile::tempdir().unwrap();
  let store = Store::new(dir.path());
  let cfg = Arc::new(config(&[("alpha", 2, false, &["codex"])]));
  for i in 0..8 {
    store
      .submit(submission(&format!("task-{i}")), b"input")
      .unwrap();
  }
  let barrier = Arc::new(Barrier::new(8));
  let handles: Vec<_> = (0..8)
    .map(|i| {
      let root = dir.path().to_owned();
      let cfg = Arc::clone(&cfg);
      let barrier = Arc::clone(&barrier);
      std::thread::spawn(move || {
        barrier.wait();
        Store::new(&root)
          .reserve(&format!("task-{i}"), &cfg, 10)
          .unwrap()
          .is_some()
      })
    })
    .collect();
  assert_eq!(
    handles
      .into_iter()
      .map(|handle| handle.join().unwrap())
      .filter(|reserved| *reserved)
      .count(),
    2
  );
  assert_eq!(
    store
      .list()
      .unwrap()
      .iter()
      .filter(|task| task.phase == "preparing")
      .count(),
    2
  );
}

#[test]
fn retry_persists_backoff_and_changes_attempt_but_not_operation() {
  let dir = tempfile::tempdir().unwrap();
  let store = Store::new(dir.path());
  let cfg = config(&[("alpha", 1, false, &["codex"])]);
  let queued = store.submit(submission("one"), b"input").unwrap();
  let first = store.reserve("one", &cfg, 100).unwrap().unwrap();
  let attempt = first.attempt.as_deref().unwrap();
  let retry = store.retry("one", attempt, 100, "transient").unwrap();
  assert_eq!(retry.phase, "queued");
  assert_eq!(retry.failures, 1);
  assert!(retry.next_at > 100);
  let reopened = Store::new(dir.path());
  assert_eq!(reopened.load("one").unwrap().next_at, retry.next_at);
  assert!(
    reopened
      .reserve("one", &cfg, retry.next_at - 1)
      .unwrap()
      .is_none()
  );
  assert!(
    reopened
      .reserve("one", &cfg, retry.next_at)
      .unwrap()
      .is_none()
  );
  let second = reopened
    .reserve("one", &cfg, retry.next_at + (retry.next_at - 100))
    .unwrap()
    .unwrap();
  assert_eq!(second.operation, queued.operation);
  assert_ne!(second.attempt.as_deref(), Some(attempt));
}

#[test]
fn cancellation_holds_capacity_until_finished_and_is_repeatable() {
  let dir = tempfile::tempdir().unwrap();
  let store = Store::new(dir.path());
  let cfg = config(&[("alpha", 1, false, &["codex"])]);
  store.submit(submission("one"), b"input").unwrap();
  store.submit(submission("two"), b"input").unwrap();
  let running = store.reserve("one", &cfg, 10).unwrap().unwrap();
  let attempt = running.attempt.as_deref().unwrap();
  assert_eq!(store.cancel("one").unwrap().phase, "cancelling");
  assert_eq!(store.cancel("one").unwrap().phase, "cancelling");
  assert!(store.reserve("two", &cfg, 10).unwrap().is_none());
  assert!(store.finish_cancel("one", "stale").is_err());
  assert_eq!(
    store.finish_cancel("one", attempt).unwrap().phase,
    "cancelled"
  );
  assert_eq!(
    store.finish_cancel("one", attempt).unwrap().phase,
    "cancelled"
  );
  assert!(store.reserve("two", &cfg, 10).unwrap().is_some());
}

#[test]
fn report_rejects_stale_or_conflicting_receipts_and_replays_identical_result() {
  let dir = tempfile::tempdir().unwrap();
  let store = Store::new(dir.path());
  let cfg = config(&[("alpha", 1, false, &["codex"])]);
  store.submit(submission("one"), b"input").unwrap();
  let task = store.reserve("one", &cfg, 10).unwrap().unwrap();
  let attempt = task.attempt.as_deref().unwrap();
  let commit = "b".repeat(40);
  assert!(store.report("one", "stale", &commit, b"result").is_err());
  let done = store.report("one", attempt, &commit, b"result").unwrap();
  assert_eq!(done.phase, "done");
  let replay = Store::new(dir.path())
    .report("one", attempt, &commit, b"result")
    .unwrap();
  assert_eq!(replay.phase, done.phase);
  assert_eq!(replay.result, done.result);
  assert!(
    store
      .report("one", attempt, &"c".repeat(40), b"result")
      .is_err()
  );
  assert!(store.report("one", attempt, &commit, b"other").is_err());
}

#[test]
fn held_attempt_keeps_capacity_and_cannot_retry() {
  let dir = tempfile::tempdir().unwrap();
  let store = Store::new(dir.path());
  let cfg = config(&[("alpha", 1, false, &["codex"])]);
  store.submit(submission("one"), b"input").unwrap();
  store.submit(submission("two"), b"input").unwrap();
  let task = store.reserve("one", &cfg, 1).unwrap().unwrap();
  let attempt = task.attempt.as_deref().unwrap();
  assert_eq!(
    store
      .launched("one", attempt, None, "uncertain")
      .unwrap()
      .phase,
    "held"
  );
  assert!(store.retry("one", attempt, 2, "uncertain").is_err());
  assert!(store.reserve("two", &cfg, 2).unwrap().is_none());
}

#[test]
fn group_serializes_across_workers_until_completion() {
  let dir = tempfile::tempdir().unwrap();
  let store = Store::new(dir.path());
  let cfg = config(&[("alpha", 2, false, &["codex"])]);
  let mut first = submission("one");
  first.group = Some("project".into());
  let mut second = submission("two");
  second.group = first.group.clone();
  store.submit(first, b"input").unwrap();
  store.submit(second, b"input").unwrap();
  let task = store.reserve("one", &cfg, 1).unwrap().unwrap();
  assert!(store.reserve("two", &cfg, 1).unwrap().is_none());
  store
    .report(
      "one",
      task.attempt.as_deref().unwrap(),
      &"b".repeat(40),
      b"result",
    )
    .unwrap();
  assert!(store.reserve("two", &cfg, 1).unwrap().is_some());
}
