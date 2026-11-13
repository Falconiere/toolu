use crate::paths::Paths;
use crate::server::{Engine, Fault};

#[test]
fn three_queued_journal_steps_share_one_flush() {
  use std::sync::Arc;
  use std::sync::atomic::{AtomicU64, Ordering};

  let tmp = tempfile::tempdir().expect("temp");
  let mut engine = Engine::open(Paths::at(tmp.path()), None, Fault::None).expect("open");
  let sink = Arc::new(AtomicU64::new(0));
  crate::journal::index::FLUSH_SINK.with(|slot| {
    *slot.borrow_mut() = Some(Arc::clone(&sink));
  });
  for name in ["one", "two", "three"] {
    engine
      .world
      .outbox
      .push_back(crate::model::Step::Journal(crate::journal::Record::new(
        "action", name, "k", "t", "n",
      )));
  }
  engine.flush().expect("flush");
  crate::journal::index::FLUSH_SINK.with(|slot| *slot.borrow_mut() = None);
  let rows = crate::journal::tail(&engine.paths.journal_dir(), engine.now).expect("tail");
  assert_eq!(
    rows.iter().map(|row| row.seq).collect::<Vec<_>>(),
    vec![1, 2, 3]
  );
  assert_eq!(sink.load(Ordering::Relaxed), 1);
}

#[test]
fn a_failed_journal_batch_puts_the_drained_rows_back() {
  let tmp = tempfile::tempdir().expect("temp");
  let mut engine = Engine::open(Paths::at(tmp.path()), None, Fault::None).expect("open");
  for name in ["one", "two"] {
    engine
      .world
      .outbox
      .push_back(crate::model::Step::Journal(crate::journal::Record::new(
        "action", name, "k", "t", "n",
      )));
  }
  std::fs::create_dir_all(engine.paths.journal_lock()).expect("block lock");
  assert!(engine.flush().is_err());
  assert_eq!(engine.world.outbox.len(), 2);
  let names: Vec<&str> = engine
    .world
    .outbox
    .iter()
    .filter_map(|step| match step {
      crate::model::Step::Journal(row) => Some(row.name.as_str()),
      crate::model::Step::Call { .. }
      | crate::model::Step::Status { .. }
      | crate::model::Step::Issue { .. }
      | crate::model::Step::Prompt { .. }
      | crate::model::Step::Notify { .. } => None,
    })
    .collect();
  assert_eq!(names, ["one", "two"]);
  std::fs::remove_dir(engine.paths.journal_lock()).expect("restore lock");
}

#[test]
fn journal_batch_follow_ups_run_after_the_shared_flush() {
  use std::sync::{Arc, Mutex};

  let tmp = tempfile::tempdir().expect("temp");
  let epic = tmp.path().join("epic");
  std::fs::create_dir_all(epic.join("issues")).expect("issues");
  let mut engine = Engine::open(Paths::at(tmp.path()), None, Fault::None).expect("open");
  let phases = Arc::new(Mutex::new(Vec::new()));
  crate::journal::index::BATCH_SINK.with(|slot| {
    *slot.borrow_mut() = Some(Arc::clone(&phases));
  });
  let state = epic.display().to_string();
  for key in ["a", "b"] {
    let issue = crate::logic::ensure_issue(&mut engine.world, key, "epic", &state);
    issue.pending = Some(crate::model::fresh(
      crate::model::Action::Launch,
      "t".into(),
    ));
    engine
      .world
      .outbox
      .push_back(crate::model::Step::Journal(crate::journal::Record::new(
        "action",
        "launch-done",
        key,
        "t",
        "",
      )));
  }
  engine.flush().expect("flush");
  crate::journal::index::BATCH_SINK.with(|slot| *slot.borrow_mut() = None);
  let seen = phases.lock().expect("phases").clone();
  assert_eq!(seen, ["flushed", "follow", "follow"]);
}
