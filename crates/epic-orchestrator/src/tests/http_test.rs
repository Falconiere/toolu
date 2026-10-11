use std::io::{Read as _, Write as _};
use std::net::{SocketAddr, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};

use toolu_runtime::config::secrets::Secrets;

use super::{Config, start};
use crate::model::World;
use crate::paths::Paths;

fn request(addr: SocketAddr, method: &str, host: &str) -> String {
  let mut stream = TcpStream::connect(addr).expect("connect");
  let text =
    format!("{method} /api/v1/overview HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n");
  stream.write_all(text.as_bytes()).expect("request");
  let mut bytes = Vec::new();
  stream.read_to_end(&mut bytes).expect("response");
  String::from_utf8(bytes).expect("utf-8")
}

#[test]
fn http_boundary_refuses_public_bind_without_a_token() {
  let temp = tempfile::tempdir().expect("temp root");
  let config = Config {
    addr: "0.0.0.0:0".parse().expect("address"),
    secrets: Secrets::none(),
  };
  let result = start(
    config,
    Paths::at(temp.path()),
    Arc::new(Mutex::new(World::new(0))),
    Arc::new(Condvar::new()),
    Arc::new(AtomicBool::new(false)),
  );
  assert!(
    result
      .err()
      .expect("refused")
      .contains("requires TOOLU_EPIC_TOKEN")
  );
}

#[test]
fn http_boundary_rejects_foreign_host_and_non_get_methods() {
  let temp = tempfile::tempdir().expect("temp root");
  let stop = Arc::new(AtomicBool::new(false));
  let config = Config {
    addr: "127.0.0.1:0".parse().expect("address"),
    secrets: Secrets::none(),
  };
  let running = start(
    config,
    Paths::at(temp.path()),
    Arc::new(Mutex::new(World::new(0))),
    Arc::new(Condvar::new()),
    Arc::clone(&stop),
  )
  .expect("listener");
  let foreign = request(running.addr, "GET", "foreign.example");
  let denied = request(running.addr, "POST", &running.addr.to_string());
  let allowed = request(running.addr, "GET", &running.addr.to_string());
  assert!(foreign.starts_with("HTTP/1.1 403"));
  assert!(denied.starts_with("HTTP/1.1 405"));
  assert!(denied.contains("Allow: GET, HEAD"));
  assert!(allowed.starts_with("HTTP/1.1 200"));
  assert!(allowed.contains("\"schema\":1"));
  assert!(!allowed.contains("Access-Control-Allow-Origin"));
  stop.store(true, Ordering::Relaxed);
  running.thread.join().expect("listener thread");
}
