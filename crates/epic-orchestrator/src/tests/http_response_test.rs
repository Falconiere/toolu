use std::io::Read as _;
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;

use super::{OverviewCache, Response, send, stream_headers};
use crate::model::World;
use crate::paths::Paths;
use toolu_runtime::config::secrets::Secrets;

#[test]
fn overview_cache_follows_the_world_generation() {
  let root = tempfile::tempdir().expect("resource root");
  let paths = Paths::at(root.path());
  let cache = OverviewCache::new();
  let mut world = World::new(0);
  let first = cache.response(&world, &paths, &Secrets::none());
  assert!(String::from_utf8_lossy(&first.body).contains("\"journal_seq\":0"));
  thread::sleep(Duration::from_millis(150));
  let again = cache.response(&world, &paths, &Secrets::none());
  assert_eq!(again.body, first.body);
  assert_eq!(cache.serializations.load(Ordering::Relaxed), 1);
  world.seq = 7;
  let stale = cache.response(&world, &paths, &Secrets::none());
  assert_eq!(stale.body, first.body);
  world.generation = world.generation.wrapping_add(1);
  let second = cache.response(&world, &paths, &Secrets::none());
  assert!(String::from_utf8_lossy(&second.body).contains("\"journal_seq\":7"));
  assert_eq!(cache.serializations.load(Ordering::Relaxed), 2);
}

#[test]
fn poisoned_overview_cache_returns_unavailable() {
  let cache = OverviewCache::new();
  let _poisoned = std::panic::catch_unwind(|| {
    let _guard = cache.current.lock().expect("cache lock");
    panic!("poison cache");
  });
  let root = tempfile::tempdir().expect("resource root");
  let response = cache.response(&World::new(0), &Paths::at(root.path()), &Secrets::none());
  assert_eq!(response.status, 500);
  assert!(String::from_utf8_lossy(&response.body).contains("status_unavailable"));
}

#[test]
fn http_boundary_error_has_no_cors_and_is_not_cached() {
  let response = socket_response(|stream| {
    send(stream, &Response::error(403, "foreign_host")).expect("send");
  });
  assert!(response.starts_with("HTTP/1.1 403 Forbidden"));
  assert!(response.contains("Cache-Control: no-store"));
  assert!(
    !response
      .to_ascii_lowercase()
      .contains("access-control-allow-origin")
  );
  assert!(response.contains("foreign_host"));
}

#[test]
fn http_boundary_stream_headers_disable_caching() {
  let response = socket_response(|stream| {
    stream_headers(stream).expect("headers");
  });
  assert!(response.contains("Content-Type: text/event-stream"));
  assert!(response.contains("Cache-Control: no-store"));
}

fn socket_response(send_response: impl FnOnce(&mut TcpStream)) -> String {
  let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
  let addr = listener.local_addr().expect("address");
  let client = std::thread::spawn(move || {
    let mut stream = TcpStream::connect(addr).expect("connect");
    let mut bytes = Vec::new();
    stream.read_to_end(&mut bytes).expect("response");
    String::from_utf8(bytes).expect("utf-8")
  });
  let (mut stream, _) = listener.accept().expect("accept");
  send_response(&mut stream);
  drop(stream);
  client.join().expect("client")
}
