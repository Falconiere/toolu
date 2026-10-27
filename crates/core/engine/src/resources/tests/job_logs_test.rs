use std::fs;

use super::{MAX_BYTES, list, read, start};

#[test]
fn a_trimmed_log_keeps_its_tail_and_logical_cursor() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path();
  let writer = start(root, "cap", "worker", &["printf".to_owned()]).unwrap();
  writer.append(&vec![b'a'; MAX_BYTES]).unwrap();
  writer.append(b"TAIL").unwrap();
  let log = list(root, "worker").unwrap().pop().unwrap();
  assert_eq!(
    fs::metadata(root.join("jobs/logs/cap.log")).unwrap().len(),
    MAX_BYTES as u64
  );
  assert_eq!(log.size, MAX_BYTES as u64 + 4);
  let first = read(&log, Some(0), 16).unwrap();
  assert!(first.truncated);
  assert_eq!(first.offset, 4);
  assert_eq!(first.next_offset, 20);
  let tail = read(&log, Some(MAX_BYTES as u64), 16).unwrap();
  assert_eq!(tail.bytes, b"TAIL");
  writer.append(&vec![b'b'; MAX_BYTES + 3]).unwrap();
  let log = list(root, "worker").unwrap().pop().unwrap();
  assert_eq!(log.size, (2 * MAX_BYTES + 7) as u64);
  let wrapped = read(&log, Some(log.size - 16), 16).unwrap();
  assert_eq!(wrapped.bytes, vec![b'b'; 16]);
}
