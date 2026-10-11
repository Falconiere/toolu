use std::io::{BufRead as _, BufReader, Read as _, Write as _};
use std::net::{SocketAddr, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant, UNIX_EPOCH};

use serde_json::json;
use toolu_runtime::config::secrets;
use toolu_runtime::config::secrets::Secrets;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;

use super::{Config, start};
use crate::PROTOCOL;
use crate::client::exchange_retry;
use crate::disk::write_value;
use crate::journal::{self, Record};
use crate::model::World;
use crate::paths::Paths;
use crate::server::Fault;
use crate::socket::{ServeOptions, serve_with_http};

fn request(addr: SocketAddr, method: &str, host: &str) -> String {
  raw(
    addr,
    &format!("{method} /api/v1/overview HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n"),
  )
}

fn raw(addr: SocketAddr, text: &str) -> String {
  let mut stream = TcpStream::connect(addr).expect("connect");
  stream.write_all(text.as_bytes()).expect("request");
  let mut bytes = Vec::new();
  stream.read_to_end(&mut bytes).expect("response");
  String::from_utf8(bytes).expect("utf-8")
}

#[test]
fn http_auth_requires_a_bearer_token_on_public_bind() {
  let temp = tempfile::tempdir().expect("temp root");
  let env = Env::from_pairs([
    ("HOME", temp.path().display().to_string()),
    (
      "TOOLU_EPIC_STATUS_TOKEN",
      "PRIVATE-STATUS-CANARY".to_owned(),
    ),
  ]);
  let secrets = secrets::load(&Roots::new(env, None)).expect("secrets");
  let stop = Arc::new(AtomicBool::new(false));
  let running = start(
    Config {
      addr: "0.0.0.0:0".parse().expect("address"),
      secrets,
    },
    Paths::at(temp.path()),
    Arc::new(Mutex::new(World::new(0))),
    Arc::new(Condvar::new()),
    Arc::clone(&stop),
  )
  .expect("listener");
  let addr = SocketAddr::from(([127, 0, 0, 1], running.addr.port()));
  let host = addr.to_string();
  let missing = request(addr, "GET", &host);
  let invalid = raw(
    addr,
    &format!(
      "GET /healthz HTTP/1.1\r\nHost: {host}\r\nAuthorization: Bearer wrong\r\nConnection: close\r\n\r\n"
    ),
  );
  let valid = raw(
    addr,
    &format!(
      "GET /healthz HTTP/1.1\r\nHost: {host}\r\nAuthorization: Bearer PRIVATE-STATUS-CANARY\r\nConnection: close\r\n\r\n"
    ),
  );
  assert!(missing.starts_with("HTTP/1.1 401"));
  assert!(invalid.starts_with("HTTP/1.1 401"));
  assert!(valid.starts_with("HTTP/1.1 200"));
  for response in [missing, invalid, valid] {
    assert!(!response.contains("PRIVATE-STATUS-CANARY"));
  }
  stop.store(true, Ordering::Relaxed);
  running.thread.join().expect("listener thread");
}

#[test]
fn http_boundary_checks_requests_and_poisoned_world() {
  let temp = tempfile::tempdir().expect("temp root");
  let stop = Arc::new(AtomicBool::new(false));
  let world = Arc::new(Mutex::new(World::new(0)));
  let config = Config {
    addr: "127.0.0.1:0".parse().expect("address"),
    secrets: Secrets::none(),
  };
  let running = start(
    config,
    Paths::at(temp.path()),
    Arc::clone(&world),
    Arc::new(Condvar::new()),
    Arc::clone(&stop),
  )
  .expect("listener");
  let foreign = request(running.addr, "GET", "foreign.example");
  let denied = request(running.addr, "POST", &running.addr.to_string());
  let allowed = request(running.addr, "GET", &running.addr.to_string());
  let head = request(running.addr, "HEAD", &running.addr.to_string());
  let bad_cursor = raw(
    running.addr,
    &format!(
      "GET /api/v1/events?since=bad HTTP/1.1\r\nHost: {}\r\n\r\n",
      running.addr
    ),
  );
  let stream_head = raw(
    running.addr,
    &format!(
      "HEAD /api/v1/events/stream HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
      running.addr
    ),
  );
  assert!(foreign.starts_with("HTTP/1.1 403"));
  assert!(denied.starts_with("HTTP/1.1 405"));
  assert!(denied.contains("Allow: GET, HEAD"));
  assert!(allowed.starts_with("HTTP/1.1 200"));
  assert!(allowed.contains("\"schema\":1"));
  assert!(head.starts_with("HTTP/1.1 200"));
  assert!(head.ends_with("\r\n\r\n"));
  assert!(bad_cursor.starts_with("HTTP/1.1 400"));
  assert!(bad_cursor.contains("\"error\":\"bad_cursor\""));
  assert!(stream_head.starts_with("HTTP/1.1 200"));
  assert!(stream_head.contains("text/event-stream"));
  assert!(stream_head.ends_with("\r\n\r\n"));
  assert!(!allowed.contains("Access-Control-Allow-Origin"));
  assert_poisoned_world(world, running.addr);
  stop.store(true, Ordering::Relaxed);
  running.thread.join().expect("listener thread");
}

fn assert_poisoned_world(world: Arc<Mutex<World>>, addr: SocketAddr) {
  let _poisoned = thread::spawn(move || {
    let _guard = world.lock().expect("world lock");
    panic!("poison world");
  })
  .join();
  for (method, path) in [
    ("GET", "/api/v1/overview"),
    ("HEAD", "/api/v1/issues/example"),
  ] {
    let response = raw(
      addr,
      &format!("{method} {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n"),
    );
    assert!(response.starts_with("HTTP/1.1 500"), "{response}");
    assert_eq!(response.contains("status_unavailable"), method == "GET");
  }
}

#[test]
fn stream_replay_crosses_journal_days_and_honors_last_event_id() {
  let temp = tempfile::tempdir().expect("temp root");
  let paths = Paths::at(temp.path());
  seed_journal(&paths);
  let stop = Arc::new(AtomicBool::new(false));
  let running = start(
    Config {
      addr: "127.0.0.1:0".parse().expect("address"),
      secrets: Secrets::none(),
    },
    paths,
    Arc::new(Mutex::new(World::new(0))),
    Arc::new(Condvar::new()),
    Arc::clone(&stop),
  )
  .expect("listener");
  let mut stream = TcpStream::connect(running.addr).expect("connect");
  stream
    .set_read_timeout(Some(Duration::from_secs(2)))
    .expect("timeout");
  let request = format!(
    "GET /api/v1/events/stream?since=0 HTTP/1.1\r\nHost: {}\r\nLast-Event-ID: 1\r\n\r\n",
    running.addr
  );
  stream
    .write_all(request.as_bytes())
    .expect("stream request");
  let reader = BufReader::new(stream);
  let (ids, seen_private) = event_ids(reader);
  assert_eq!(ids, [2, 3]);
  assert!(!seen_private);
  stop.store(true, Ordering::Relaxed);
  running.thread.join().expect("listener thread");
}

fn seed_journal(paths: &Paths) {
  for (day, name) in [(0, "first"), (86_400, "second"), (172_800, "third")] {
    journal::append(
      &paths.journal_dir(),
      &paths.journal_lock(),
      &Record::new(
        "transition",
        name,
        "repo-449",
        "PRIVATE-TOKEN",
        "phase changed",
      ),
      UNIX_EPOCH + Duration::from_secs(day),
    )
    .expect("journal event");
  }
}

fn event_ids(mut reader: BufReader<TcpStream>) -> (Vec<u64>, bool) {
  let mut ids = Vec::new();
  let mut seen_private = false;
  for _ in 0..20 {
    let mut line = String::new();
    reader.read_line(&mut line).expect("stream line");
    seen_private |= line.contains("PRIVATE-TOKEN");
    if let Some(id) = line.strip_prefix("id: ") {
      ids.push(id.trim().parse::<u64>().expect("event id"));
    }
    if ids.len() == 2 {
      break;
    }
  }
  (ids, seen_private)
}

#[test]
fn dashboard_live_report_reaches_http_within_two_seconds() {
  let temp = tempfile::tempdir().expect("temp root");
  let (paths, epic, addr, engine) = live_fixture(temp.path());
  let report = json!({"op": "report", "key": "repo-449", "epic": "repo-402",
    "state_dir": epic.display().to_string(), "phase": "execution", "note": "working"});
  let start = Instant::now();
  assert_eq!(
    exchange_retry(&paths, PROTOCOL, &report).expect("report")["ok"],
    true
  );
  let mut body = String::new();
  while start.elapsed() < Duration::from_secs(2) {
    body = raw(
      addr,
      &format!("GET /api/v1/issues/repo-449 HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n"),
    );
    if body.contains("\"phase\":\"execution\"") {
      break;
    }
    thread::sleep(Duration::from_millis(10));
  }
  assert!(body.contains("\"phase\":\"execution\""), "{body}");
  assert!(start.elapsed() < Duration::from_secs(2));
  exchange_retry(&paths, PROTOCOL, &json!({"op": "stop"})).expect("stop");
  engine
    .join()
    .expect("engine thread")
    .expect("engine result");
}

fn live_fixture(
  root: &std::path::Path,
) -> (
  Paths,
  std::path::PathBuf,
  SocketAddr,
  thread::JoinHandle<Result<(), String>>,
) {
  let paths = Paths::at(root.join("resources"));
  let epic = root.join("epic-402");
  std::fs::create_dir_all(epic.join("status")).expect("status dir");
  write_value(
    &paths.registry(),
    &json!({"epics": [{"key": "repo-402", "state_dir": epic}]}),
  )
  .expect("registry");
  write_value(
    &epic.join("graph.json"),
    &json!({"issues": [{"key": "repo-449", "title": "Status", "wave": 1, "open_blockers": []}]}),
  )
  .expect("graph");
  let reserved = std::net::TcpListener::bind("127.0.0.1:0").expect("port");
  let addr = reserved.local_addr().expect("address");
  drop(reserved);
  let background = paths.clone();
  let engine = thread::spawn(move || {
    serve_with_http(
      background,
      None,
      Fault::None,
      PROTOCOL,
      ServeOptions {
        http: Some(Config {
          addr,
          secrets: Secrets::none(),
        }),
        tick: &(),
      },
    )
  });
  for _ in 0..100 {
    if paths.socket().exists() {
      break;
    }
    thread::sleep(Duration::from_millis(10));
  }
  (paths, epic, addr, engine)
}
