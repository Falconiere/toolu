use std::os::unix::net::UnixListener;
use std::time::{Duration, Instant};

use super::{exchange_retry, spool_path, with_protocol};
use crate::paths::Paths;

#[test]
fn a_request_keeps_its_fields_and_records_the_protocol() {
  let body = with_protocol(&serde_json::json!({"op": "wait", "max_seconds": 0}), 1);
  assert_eq!(body["op"], "wait");
  assert_eq!(body["protocol"], 1);
  let path = spool_path(std::path::Path::new("/tmp/epic"), "t1").expect("token");
  assert_eq!(path, std::path::PathBuf::from("/tmp/epic/spool/t1.json"));
  assert!(spool_path(std::path::Path::new("/tmp/epic"), "../x").is_err());
}

#[test]
fn a_dropped_connection_is_not_retried() {
  let tmp = tempfile::tempdir().expect("temp");
  let paths = Paths::at(tmp.path());
  let listener = UnixListener::bind(paths.socket()).expect("bind");
  std::thread::spawn(move || {
    let _accepted = listener.accept();
  });
  let started = Instant::now();
  let err = exchange_retry(&paths, 1, &serde_json::json!({"op": "status"})).expect_err("closed");
  assert!(started.elapsed() < Duration::from_millis(400), "{err}");
}
