use super::host_allowed;
use crate::http::{Config, start};
use crate::model::World;
use crate::paths::Paths;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Condvar, Mutex};
use toolu_runtime::config::secrets::Secrets;

#[test]
fn loopback_accepts_only_its_own_port_and_names() {
  let addr = "127.0.0.1:7717".parse().expect("address");
  assert!(host_allowed("localhost:7717", addr));
  assert!(host_allowed("127.0.0.1:7717", addr));
  assert!(!host_allowed("localhost:7718", addr));
  assert!(!host_allowed("foreign.example:7717", addr));
  // What a `::` bind reports for a connection to 127.0.0.1, as `token url` prints.
  let mapped = "[::ffff:127.0.0.1]:7717".parse().expect("mapped address");
  assert!(host_allowed("127.0.0.1:7717", mapped));
  let public = "[::ffff:192.0.2.1]:7717".parse().expect("mapped address");
  assert!(!host_allowed("127.0.0.1:7717", public));
}

#[test]
fn http_boundary_refuses_public_bind_without_a_token() {
  let temp = tempfile::tempdir().expect("temp root");
  let result = start(
    Config::new(
      "0.0.0.0:0".parse().expect("address"),
      Secrets::none(),
      Vec::new(),
    ),
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
