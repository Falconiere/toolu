use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use super::{fed, feed, take_pipes};

#[test]
fn a_canceled_feeder_releases_a_real_unread_pipe() {
  let mut child = Command::new("sleep")
    .arg("30")
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .unwrap();
  let (stdin, _stdout, _stderr) = take_pipes(&mut child).unwrap();
  let cancel = Arc::new(AtomicBool::new(false));
  let feeder = feed(stdin, vec![b'x'; 8 * 1024 * 1024], Arc::clone(&cancel)).unwrap();
  std::thread::sleep(Duration::from_millis(20));
  cancel.store(true, Ordering::Release);
  let result = fed(feeder, Instant::now() + Duration::from_millis(250));
  child.kill().unwrap();
  child.wait().unwrap();
  assert_eq!(result, Some(Ok(())));
}
