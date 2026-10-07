use std::time::Duration;

use crate::Reply;

#[test]
fn a_new_reply_answers_at_once_with_no_header() {
  let reply = Reply::new(200, "ok");
  assert_eq!(
    (reply.status, reply.body, reply.delay, reply.dropped),
    (200, b"ok".to_vec(), Duration::ZERO, false)
  );
  assert_eq!(reply.headers.len(), 0);
}

#[test]
fn headers_and_delay_accumulate() {
  let reply = Reply::new(429, "busy")
    .header("Retry-After", "1")
    .header("X-Test", "a")
    .delayed(Duration::from_millis(5));
  assert_eq!(
    reply.headers,
    [
      ("Retry-After".to_owned(), "1".to_owned()),
      ("X-Test".to_owned(), "a".to_owned())
    ]
  );
  assert_eq!(reply.delay, Duration::from_millis(5));
}

#[test]
fn a_dropped_reply_has_no_response() {
  let reply = Reply::dropped();
  assert!(reply.dropped);
  assert_eq!(reply.body.len(), 0);
}
