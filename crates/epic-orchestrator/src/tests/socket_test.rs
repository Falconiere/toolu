use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use serde_json::{Value, json};

use super::serve;
use crate::PROTOCOL;
use crate::client::{exchange_retry, read_json, write_json};
use crate::journal;
use crate::paths::Paths;
use crate::server::Fault;

fn epic_dir(root: &std::path::Path) -> std::path::PathBuf {
  let epic = root.join("epic");
  std::fs::create_dir_all(epic.join("status")).expect("status dir");
  epic
}

fn report(epic: &std::path::Path, token: &str, phase: &str) -> Value {
  json!({
    "op": "report",
    "token": token,
    "key": "a",
    "epic": "one",
    "state_dir": epic.display().to_string(),
    "phase": phase,
  })
}

fn serve_until_stopped(paths: Paths, protocol: u64) {
  let handle = thread::spawn(move || serve(paths, None, Fault::None, protocol, &()));
  handle.join().expect("serve thread").expect("serve");
}

fn stop(paths: &Paths, protocol: u64) {
  let stopped = exchange_retry(paths, protocol, &json!({"op": "stop"})).expect("stop");
  assert_eq!(stopped["ok"], true);
}

fn phase_count(paths: &Paths) -> usize {
  journal::tail(&paths.journal_dir(), std::time::SystemTime::now())
    .expect("journal")
    .into_iter()
    .filter(|row| row.name == "phase" && row.key == "a")
    .count()
}

#[test]
fn wait_limit() {
  let tmp = tempfile::tempdir().expect("temp");
  let paths = Paths::at(tmp.path());
  let background = paths.clone();
  let handle = thread::spawn(move || serve_until_stopped(background, PROTOCOL));
  let body =
    exchange_retry(&paths, PROTOCOL, &json!({"op": "wait", "max_seconds": 0})).expect("wait");
  assert_eq!(body, json!({"state": "waiting"}));
  stop(&paths, PROTOCOL);
  handle.join().expect("joined");
}

#[test]
fn protocol_replace() {
  let tmp = tempfile::tempdir().expect("temp");
  let epic = epic_dir(tmp.path());
  let paths = Paths::at(tmp.path());
  let first = paths.clone();
  let old = thread::spawn(move || serve_until_stopped(first, PROTOCOL));
  let reply = exchange_retry(&paths, 2, &report(&epic, "t9", "running")).expect("replace");
  assert_eq!(reply["ok"], true);
  old.join().expect("old engine");
  let spool = paths.spool().join("t9.json");
  assert!(spool.is_file(), "spool survives replace");
  assert!(!epic.join("status").join("a.json").is_file());
  let second = paths.clone();
  let newer = thread::spawn(move || serve_until_stopped(second, 2));
  let status = epic.join("status").join("a.json");
  for _ in 0..50 {
    if status.is_file() {
      break;
    }
    thread::sleep(Duration::from_millis(20));
  }
  let text = std::fs::read_to_string(&status).expect("status");
  assert!(text.contains("\"phase\":\"running\""), "{text}");
  assert_eq!(phase_count(&paths), 1);
  stop(&paths, 2);
  newer.join().expect("new engine");
}

#[test]
fn spool_survives() {
  if std::env::var("TOOLU_EPIC_CHILD").ok().as_deref() == Some("serve") {
    let root = std::env::var("TOOLU_EPIC_ROOT").expect("root");
    let _ran = serve(Paths::at(root), None, Fault::None, PROTOCOL, &());
    return;
  }
  let tmp = tempfile::tempdir().expect("temp");
  let epic = epic_dir(tmp.path());
  let paths = Paths::at(tmp.path());
  let log = tmp.path().join("child.log");
  let mut child = hold_child(&paths, &log);
  let mut stream = wait_socket(&paths, &log);
  crate::disk::write_value(
    &paths.spool().join("t8.json"),
    &report(&epic, "t8", "blocked"),
  )
  .expect("spool");
  let hello = read_json(&mut stream).expect("hello");
  assert_eq!(hello["protocol"], PROTOCOL);
  write_json(
    &mut stream,
    &json!({"op": "report", "protocol": PROTOCOL, "token": "t8"}),
  )
  .expect("send");
  thread::sleep(Duration::from_millis(200));
  child.kill().expect("kill");
  let _status = child.wait();
  assert!(
    paths.spool().join("t8.json").is_file(),
    "spool survives SIGKILL"
  );
  assert_restart(&paths, &epic);
}

fn hold_child(paths: &Paths, log: &std::path::Path) -> std::process::Child {
  let file = std::fs::File::create(log).expect("log");
  let mut child = Command::new(std::env::current_exe().expect("exe"));
  child
    .env("TOOLU_EPIC_CHILD", "serve")
    .env("TOOLU_EPIC_HOLD", "before-ack")
    .env("TOOLU_EPIC_ROOT", paths.root.clone())
    .stdout(std::process::Stdio::from(file.try_clone().expect("stdout")))
    .stderr(std::process::Stdio::from(file))
    .arg("--test-threads=1")
    .arg("--exact")
    .arg("socket::tests::spool_survives");
  child.spawn().expect("child")
}

fn assert_restart(paths: &Paths, epic: &std::path::Path) {
  let background = paths.clone();
  let handle = thread::spawn(move || serve_until_stopped(background, PROTOCOL));
  let status = epic.join("status").join("a.json");
  for _ in 0..50 {
    if status.is_file() {
      break;
    }
    thread::sleep(Duration::from_millis(20));
  }
  let text = std::fs::read_to_string(&status).expect("applied");
  assert!(text.contains("\"phase\":\"blocked\""), "{text}");
  assert_eq!(phase_count(paths), 1);
  let spool = paths.spool().join("t8.json");
  for _ in 0..50 {
    if !spool.is_file() {
      break;
    }
    thread::sleep(Duration::from_millis(20));
  }
  assert!(!spool.is_file(), "spooled t8.json left behind");
  stop(paths, PROTOCOL);
  handle.join().expect("restart");
}

#[test]
fn a_panic_in_the_state_loop_sets_the_accept_stop_flag() {
  let stop = AtomicBool::new(false);
  let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
    let _stop_accept = super::StopAccept { stop: &stop };
    panic!("state_loop");
  }));
  assert!(caught.is_err());
  assert!(stop.load(Ordering::Relaxed));
}

fn wait_socket(paths: &Paths, log: &std::path::Path) -> std::os::unix::net::UnixStream {
  for _ in 0..100 {
    if let Ok(stream) = std::os::unix::net::UnixStream::connect(paths.socket()) {
      return stream;
    }
    thread::sleep(Duration::from_millis(20));
  }
  let detail = std::fs::read_to_string(log).unwrap_or_default();
  panic!("socket did not open\n{detail}");
}
