use std::io::Read as _;
use std::net::{TcpListener, TcpStream};

use super::{OverviewCache, Response, send, stream_headers};
use crate::model::World;
use crate::paths::Paths;
use toolu_runtime::config::secrets::Secrets;

#[test]
fn overview_cache_refreshes_when_world_sequence_changes() {
  let root = tempfile::tempdir().expect("resource root");
  let paths = Paths::at(root.path());
  let cache = OverviewCache::new();
  let mut world = World::new(0);
  let first = cache.response(&world, &paths, &Secrets::none());
  assert!(String::from_utf8_lossy(&first.body).contains("\"journal_seq\":0"));
  world.seq = 7;
  let second = cache.response(&world, &paths, &Secrets::none());
  assert!(String::from_utf8_lossy(&second.body).contains("\"journal_seq\":7"));
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
