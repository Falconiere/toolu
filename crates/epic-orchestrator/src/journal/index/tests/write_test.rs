#[test]
fn the_synced_record_is_a_single_terminated_line() {
  let root = tempfile::tempdir().unwrap();
  let dir = root.path().join("out");
  let lock = root.path().join("mu");
  let at = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_400_000_000);
  let row = crate::journal::Record::new("action", "write", "k", "t", "n");
  let stored = crate::journal::append(&dir, &lock, &row, at).unwrap();
  let text = std::fs::read_to_string(crate::journal::file_for(&dir, at)).unwrap();
  assert_eq!(text.chars().filter(|ch| *ch == '\n').count(), 1);
  assert!(text.contains(&format!("\"seq\":{}", stored.seq)));
}

#[test]
fn each_append_flushes_only_its_jsonl_line() {
  use std::sync::Arc;
  use std::sync::atomic::{AtomicU64, Ordering};

  let root = tempfile::tempdir().unwrap();
  let dir = root.path().join("journal");
  let lock = root.path().join("journal.lock");
  let at = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
  let sink = Arc::new(AtomicU64::new(0));
  super::super::FLUSH_SINK.with(|slot| *slot.borrow_mut() = Some(Arc::clone(&sink)));
  let _disarm = FlushDisarm;
  for _ in 0..2 {
    crate::journal::append(
      &dir,
      &lock,
      &crate::journal::Record::new("action", "flush-count", "k", "t", "n"),
      at,
    )
    .unwrap();
    assert_eq!(sink.swap(0, Ordering::Relaxed), 1);
  }
}

struct FlushDisarm;

impl Drop for FlushDisarm {
  fn drop(&mut self) {
    super::super::FLUSH_SINK.with(|slot| *slot.borrow_mut() = None);
  }
}

#[test]
fn append_batch_assigns_consecutive_sequences_and_flushes_once() {
  use std::sync::Arc;
  use std::sync::atomic::{AtomicU64, Ordering};

  let root = tempfile::tempdir().unwrap();
  let dir = root.path().join("journal");
  let lock = root.path().join("journal.lock");
  let at = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_710_000_000);
  let sink = Arc::new(AtomicU64::new(0));
  super::super::FLUSH_SINK.with(|slot| *slot.borrow_mut() = Some(Arc::clone(&sink)));
  let _disarm = FlushDisarm;
  let rows = [
    crate::journal::Record::new("action", "batch-a", "k", "t", "n"),
    crate::journal::Record::new("action", "batch-b", "k", "t", "n"),
    crate::journal::Record::new("action", "batch-c", "k", "t", "n"),
  ];
  let stored = crate::journal::append_batch(&dir, &lock, &rows, at).unwrap();
  assert_eq!(
    stored.iter().map(|row| row.seq).collect::<Vec<_>>(),
    vec![1, 2, 3]
  );
  assert_eq!(sink.load(Ordering::Relaxed), 1);
}
