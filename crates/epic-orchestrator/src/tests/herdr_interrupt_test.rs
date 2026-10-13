use std::io::Read as _;
use std::os::unix::net::UnixStream;
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::Interrupt;

#[test]
fn a_trigger_ends_a_blocked_read_on_the_shared_stream() {
  let (mut ours, _peer) = UnixStream::pair().unwrap();
  ours
    .set_read_timeout(Some(Duration::from_secs(30)))
    .unwrap();
  let interrupt = Arc::new(Interrupt::default());
  interrupt.publish(&ours);
  let trigger = Arc::clone(&interrupt);
  let started = Instant::now();
  let thread = std::thread::spawn(move || {
    std::thread::sleep(Duration::from_millis(100));
    trigger.trigger();
  });
  let mut byte = [0_u8; 1];
  assert_eq!(
    ours.read(&mut byte).unwrap(),
    0,
    "a shut-down stream reads end of file"
  );
  assert!(started.elapsed() < Duration::from_secs(5));
  thread.join().unwrap();
}

#[test]
fn a_trigger_before_publish_shuts_the_next_stream_until_reset() {
  let interrupt = Interrupt::default();
  interrupt.trigger();
  let (mut first, _peer) = UnixStream::pair().unwrap();
  interrupt.publish(&first);
  let mut byte = [0_u8; 1];
  assert_eq!(first.read(&mut byte).unwrap(), 0);
  interrupt.reset();
  let (mut second, peer) = UnixStream::pair().unwrap();
  interrupt.publish(&second);
  drop(peer);
  assert_eq!(
    second.read(&mut byte).unwrap(),
    0,
    "closed by its peer, not by a trigger"
  );
  interrupt.clear();
}

#[test]
fn a_pause_ends_on_a_trigger_or_at_its_limit() {
  let interrupt = Arc::new(Interrupt::default());
  let started = Instant::now();
  interrupt.pause(Duration::from_millis(50));
  assert!(started.elapsed() >= Duration::from_millis(50));
  let trigger = Arc::clone(&interrupt);
  let thread = std::thread::spawn(move || {
    std::thread::sleep(Duration::from_millis(50));
    trigger.trigger();
  });
  let started = Instant::now();
  interrupt.pause(Duration::from_secs(30));
  assert!(started.elapsed() < Duration::from_secs(5));
  thread.join().unwrap();
}
