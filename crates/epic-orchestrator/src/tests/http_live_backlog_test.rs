use std::os::unix::net::{UnixListener, UnixStream};
use std::time::{Duration, Instant};

use nix::sys::socket::{Backlog, listen};
use serde_json::json;

use super::{connect, request};

fn fd_count() -> usize {
  let path = if cfg!(target_os = "linux") {
    "/proc/self/fd"
  } else {
    "/dev/fd"
  };
  std::fs::read_dir(path).expect("fd directory").count()
}

#[cfg(target_os = "linux")]
fn thread_count() -> usize {
  std::fs::read_dir("/proc/self/task")
    .expect("thread directory")
    .count()
}

#[test]
fn server_disconnect_and_shutdown_leave_no_usable_connection() {
  let root = tempfile::tempdir().expect("root");
  let path = root.path().join("herdr.sock");
  let listener = UnixListener::bind(&path).expect("listener");
  let server = std::thread::spawn(move || {
    let (peer, _) = listener.accept().expect("accept");
    drop(peer);
  });
  assert_eq!(
    request(&path, "agent.list", &json!({})).expect_err("disconnect"),
    "not running"
  );
  server.join().expect("server");
  std::fs::remove_file(&path).expect("unlink shut down socket");
  assert!(connect(&path).is_err(), "shutdown listener must refuse");
}

#[cfg(target_os = "linux")]
#[test]
fn connect_recovers_when_a_full_backlog_drains_before_deadline() {
  let root = tempfile::tempdir().expect("root");
  let path = root.path().join("herdr.sock");
  let listener = UnixListener::bind(&path).expect("listener");
  listen(&listener, Backlog::new(1).expect("backlog")).expect("listen");
  let held = [
    UnixStream::connect(&path).expect("first queued"),
    UnixStream::connect(&path).expect("second queued"),
  ];
  let draining = listener.try_clone().expect("clone listener");
  let server = std::thread::spawn(move || {
    std::thread::sleep(Duration::from_millis(100));
    let (peer, _) = draining.accept().expect("drain queue");
    drop(peer);
  });
  let started = Instant::now();
  let connected = connect(&path).expect("backlog should drain");
  assert!(started.elapsed() < Duration::from_secs(1));
  drop(connected);
  server.join().expect("server");
  drop(held);
}

#[test]
fn backlog_connects_release_resources_at_deadline() {
  let root = tempfile::tempdir().expect("root");
  let path = root.path().join("herdr.sock");
  let listener = UnixListener::bind(&path).expect("listener");
  listen(&listener, Backlog::new(1).expect("backlog")).expect("listen");
  // Linux queues two connections for backlog 1; macOS queues one.
  let queued = if cfg!(target_os = "linux") { 2 } else { 1 };
  let _held: Vec<_> = (0..queued)
    .map(|_| UnixStream::connect(&path).expect("fill backlog"))
    .collect();
  let fds_before = fd_count();
  #[cfg(target_os = "linux")]
  let threads_before = thread_count();
  let started = Instant::now();
  for _ in 0..4 {
    let attempt = Instant::now();
    assert!(
      connect(&path).is_err(),
      "full backlog must refuse or time out"
    );
    assert!(
      attempt.elapsed() < Duration::from_millis(1_300),
      "connect passed its deadline"
    );
  }
  let fds_after = fd_count();
  #[cfg(target_os = "linux")]
  let threads_after = thread_count();
  println!(
    "backlog attempts=4 elapsed={:?} fds={fds_before}->{fds_after}",
    started.elapsed()
  );
  #[cfg(target_os = "linux")]
  println!("threads={threads_before}->{threads_after}");
  listener
    .set_nonblocking(true)
    .expect("nonblocking listener");
  for _ in 0..queued {
    let (peer, _) = listener.accept().expect("queued connection");
    drop(peer);
  }
  std::thread::sleep(Duration::from_millis(100));
  assert!(
    matches!(listener.accept(), Err(err) if err.kind() == std::io::ErrorKind::WouldBlock),
    "a timed-out connector reached the listener late"
  );
}
