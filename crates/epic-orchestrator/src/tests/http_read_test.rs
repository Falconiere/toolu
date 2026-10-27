use std::io::{BufRead as _, BufReader, Write as _};
use std::net::TcpStream;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, UNIX_EPOCH};

use toolu_runtime::config::secrets::Secrets;

use crate::http::{Config, start};
use crate::journal::{self, Record};
use crate::model::World;
use crate::paths::Paths;

#[test]
fn stream_replay_crosses_journal_days_and_honors_last_event_id() {
  let temp = tempfile::tempdir().expect("temp root");
  let paths = Paths::at(temp.path());
  seed_journal(&paths);
  let stop = Arc::new(AtomicBool::new(false));
  let running = start(
    Config::new(
      "127.0.0.1:0".parse().expect("address"),
      Secrets::none(),
      Vec::new(),
    ),
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
  running.stop(&stop).expect("listener thread");
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
fn a_rotated_token_ends_a_stream_opened_with_the_old_one() {
  let temp = tempfile::tempdir().expect("temp root");
  let env = toolu_runtime::env::Env::from_pairs([(
    "TOOLU_CONFIG_DIR",
    temp.path().join("config").display().to_string(),
  )]);
  let roots = toolu_runtime::host::roots::Roots::new(env, None);
  toolu_runtime::config::secrets::rotate_status_token(&roots).expect("token");
  let loaded = toolu_runtime::config::secrets::load(&roots).expect("secrets");
  let old = loaded.status_token().expect("token").to_owned();
  let stop = Arc::new(AtomicBool::new(false));
  let config = Config {
    reload: Some(roots.clone()),
    ..Config::new("0.0.0.0:0".parse().expect("address"), loaded, Vec::new())
  };
  let world = Arc::new(Mutex::new(World::new(0)));
  let running = start(
    config,
    Paths::at(temp.path()),
    world,
    Arc::new(Condvar::new()),
    Arc::clone(&stop),
  )
  .expect("listener");
  let addr = std::net::SocketAddr::from(([127, 0, 0, 1], running.addr.port()));
  let mut stream = TcpStream::connect(addr).expect("connect");
  stream
    .set_read_timeout(Some(Duration::from_secs(5)))
    .expect("timeout");
  let request = format!(
    "GET /api/v1/events/stream HTTP/1.1\r\nHost: {addr}\r\nAuthorization: Bearer {old}\r\n\r\n"
  );
  stream
    .write_all(request.as_bytes())
    .expect("stream request");
  toolu_runtime::config::secrets::rotate_status_token(&roots).expect("rotate");
  let mut reader = BufReader::new(stream);
  let mut seen = String::new();
  while !seen.contains("\"unauthorized\"") {
    let mut line = String::new();
    if reader.read_line(&mut line).expect("stream line") == 0 {
      break;
    }
    seen.push_str(&line);
  }
  assert!(seen.starts_with("HTTP/1.1 200"), "{seen}");
  assert!(
    seen.contains("event: error\ndata: {\"schema\":1,\"error\":\"unauthorized\"}"),
    "{seen}"
  );
  running.stop(&stop).expect("listener thread");
}
