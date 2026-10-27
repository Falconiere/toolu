use std::io::{Read as _, Write as _};
use std::net::{SocketAddr, TcpStream};
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Condvar, Mutex};
use std::time::SystemTime;

use serde_json::Value;
use toolu_runtime::config::secrets::Secrets;

use super::{MetricsCache, parse};
use crate::http::{Config, start};
use crate::metrics::{self, Query};
use crate::model::World;
use crate::paths::Paths;

fn fixture_root() -> tempfile::TempDir {
  let root = tempfile::tempdir().unwrap();
  let journal = Paths::at(root.path()).journal_dir();
  std::fs::create_dir_all(&journal).unwrap();
  let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/epic/metrics/journal");
  for entry in std::fs::read_dir(source).unwrap() {
    let path = entry.unwrap().path();
    std::fs::copy(&path, journal.join(path.file_name().unwrap())).unwrap();
  }
  root
}

fn get(addr: SocketAddr, target: &str) -> (String, Value) {
  let mut stream = TcpStream::connect(addr).unwrap();
  let request = format!("GET {target} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n");
  stream.write_all(request.as_bytes()).unwrap();
  let mut text = String::new();
  stream.read_to_string(&mut text).unwrap();
  let (head, body) = text.split_once("\r\n\r\n").unwrap();
  (
    head.lines().next().unwrap().to_owned(),
    serde_json::from_str(body).unwrap(),
  )
}

fn without_clock(mut doc: Value) -> Value {
  doc.as_object_mut().unwrap().remove("generated_at");
  doc
}

#[test]
fn the_endpoint_serves_the_cli_document_and_rejects_a_bad_query() {
  let root = fixture_root();
  let stop = Arc::new(AtomicBool::new(false));
  let config = Config {
    addr: "127.0.0.1:0".parse().unwrap(),
    secrets: Secrets::none(),
    peers: Vec::new(),
    herdr: crate::http_live::Herdr::Off,
  };
  let running = start(
    config,
    Paths::at(root.path()),
    Arc::new(Mutex::new(World::new(0))),
    Arc::new(Condvar::new()),
    Arc::clone(&stop),
  )
  .unwrap();
  let query = |epic: Option<&str>| Query {
    epic: epic.map(str::to_owned),
    since_ms: None,
    now_ms: toolu_state::time::epoch_millis(SystemTime::now()),
  };
  let journal = Paths::at(root.path()).journal_dir();
  let (status, all) = get(running.addr, "/api/v1/metrics");
  assert_eq!(status, "HTTP/1.1 200 OK");
  assert_eq!(
    without_clock(all),
    without_clock(metrics::document(&journal, &query(None)).unwrap())
  );
  let (_, one) = get(running.addr, "/api/v1/metrics?epic=e1");
  assert_eq!(
    without_clock(one),
    without_clock(metrics::document(&journal, &query(Some("e1"))).unwrap())
  );
  let (bad, body) = get(running.addr, "/api/v1/metrics?since=bad");
  assert_eq!(bad, "HTTP/1.1 400 Bad Request");
  assert_eq!(body["error"], "bad_query");
  running.stop(&stop).unwrap();
}

#[test]
fn a_query_is_reused_for_a_minute_and_the_cache_stays_bounded() {
  let root = fixture_root();
  let paths = Paths::at(root.path());
  let cache = MetricsCache::new();
  let first = cache.response(&paths, "", &Secrets::none());
  std::fs::remove_dir_all(paths.journal_dir()).unwrap();
  let again = cache.response(&paths, "", &Secrets::none());
  assert_eq!(first.body, again.body, "the cached body is reused");
  for day in 1..=20 {
    let response = cache.response(&paths, &format!("since=2026-10-{day:02}"), &Secrets::none());
    assert_eq!(response.status, 200);
  }
  assert_eq!(cache.entries.lock().unwrap().len(), 16);
}

#[test]
fn only_epic_and_since_are_accepted() {
  assert_eq!(parse("").unwrap(), (None, None));
  assert_eq!(
    parse("epic=e1&since=1970-01-02").unwrap(),
    (Some("e1".to_owned()), Some(86_400_000))
  );
  assert!(parse("limit=5").is_err());
  assert!(parse("since=yesterday").is_err());
}
