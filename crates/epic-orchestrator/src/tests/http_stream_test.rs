use super::cursor;
use crate::model::World;
use crate::paths::Paths;
use std::io::Read as _;
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Condvar, Mutex};
use toolu_runtime::config::secrets::Secrets;

#[test]
fn status_stream_reconnect_header_wins_over_query_cursor() {
  let parsed = cursor("since=4&limit=20", Some("7")).expect("cursor");
  assert_eq!(parsed.since, 7);
  assert_eq!(parsed.limit, 20);
  assert!(cursor("limit=501", None).is_err());
  assert!(cursor("since=bad", None).is_err());
}

#[test]
fn poisoned_stream_world_sends_fixed_error() {
  let root = tempfile::tempdir().expect("root");
  let paths = Paths::at(root.path());
  let secrets = Secrets::none();
  let world = Arc::new(Mutex::new(World::new(0)));
  let poisoned = Arc::clone(&world);
  let _poisoned = std::thread::spawn(move || {
    let _guard = poisoned.lock().expect("world lock");
    panic!("poison world");
  })
  .join();
  let changed = Arc::new(Condvar::new());
  let stop = Arc::new(AtomicBool::new(false));
  let feed = super::Feed {
    paths: &paths,
    secrets: &secrets,
    world: &world,
    changed: &changed,
    stop: &stop,
  };
  let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
  let mut client = TcpStream::connect(listener.local_addr().expect("address")).expect("connect");
  let (mut server, _) = listener.accept().expect("accept");
  assert!(!super::wait_for_change(&mut server, &feed));
  drop(server);
  let mut body = String::new();
  client.read_to_string(&mut body).expect("error frame");
  assert_eq!(
    body,
    "event: error\ndata: {\"schema\":1,\"error\":\"status_unavailable\"}\n\n"
  );
}
