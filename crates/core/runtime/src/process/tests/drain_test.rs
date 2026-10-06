use std::io::Cursor;
use std::time::{Duration, Instant};

use super::{Budget, Drain};

fn finish(drain: &Drain) {
  let deadline = Instant::now() + Duration::from_secs(5);
  while !drain.finished() {
    assert!(Instant::now() < deadline, "the drain never finished");
    std::thread::sleep(Duration::from_millis(1));
  }
}

#[test]
fn two_streams_share_one_budget() {
  let budget = Budget::new(5);
  let first = Drain::start(Some(Cursor::new(b"abc".to_vec())), &budget);
  finish(&first);
  let second = Drain::start(Some(Cursor::new(b"defgh".to_vec())), &budget);
  finish(&second);
  assert_eq!(first.text(), "abc");
  assert_eq!(second.text(), "de");
  assert!(budget.truncated());
}

#[test]
fn a_stream_within_budget_is_kept_whole_and_lossily_decoded() {
  let budget = Budget::new(100);
  let drain = Drain::start(Some(Cursor::new(vec![b'o', b'k', 0xff])), &budget);
  finish(&drain);
  assert_eq!(drain.text(), "ok\u{fffd}");
  assert!(!budget.truncated());
}

#[test]
fn a_missing_stream_is_empty_and_finished() {
  let drain = Drain::start(None::<Cursor<Vec<u8>>>, &Budget::new(1));
  assert!(drain.finished());
  assert_eq!(drain.text(), "");
}
