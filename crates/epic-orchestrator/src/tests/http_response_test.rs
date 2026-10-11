use std::io::Read as _;
use std::net::{TcpListener, TcpStream};

use super::{Response, send, stream_headers};

#[test]
fn http_boundary_error_has_no_cors_and_is_not_cached() {
  let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
  let addr = listener.local_addr().expect("address");
  let client = std::thread::spawn(move || {
    let mut stream = TcpStream::connect(addr).expect("connect");
    let mut bytes = Vec::new();
    stream.read_to_end(&mut bytes).expect("response");
    String::from_utf8(bytes).expect("utf-8")
  });
  let (mut stream, _) = listener.accept().expect("accept");
  send(&mut stream, &Response::error(403, "foreign_host")).expect("send");
  drop(stream);
  let response = client.join().expect("client");
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
  let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
  let addr = listener.local_addr().expect("address");
  let client = std::thread::spawn(move || {
    let mut stream = TcpStream::connect(addr).expect("connect");
    let mut bytes = Vec::new();
    stream.read_to_end(&mut bytes).expect("response");
    String::from_utf8(bytes).expect("utf-8")
  });
  let (mut stream, _) = listener.accept().expect("accept");
  stream_headers(&mut stream).expect("headers");
  drop(stream);
  let response = client.join().expect("client");
  assert!(response.contains("Content-Type: text/event-stream"));
  assert!(response.contains("Cache-Control: no-store"));
}
