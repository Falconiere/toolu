use std::io::{BufReader, Write as _};
use std::net::{TcpListener, TcpStream};

use super::read_request;

#[test]
fn http_boundary_parses_one_real_get_request() {
  let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
  let addr = listener.local_addr().expect("local address");
  let sender = std::thread::spawn(move || {
    let mut stream = TcpStream::connect(addr).expect("connect");
    stream
      .write_all(b"GET /api/v1/overview HTTP/1.1\r\nHost: 127.0.0.1:7717\r\n\r\n")
      .expect("send request");
  });
  let (stream, _) = listener.accept().expect("accept");
  let request = read_request(&mut BufReader::new(stream))
    .expect("parse")
    .expect("request");
  sender.join().expect("sender");
  assert_eq!(request.method, "GET");
  assert_eq!(request.target, "/api/v1/overview");
  assert_eq!(request.host, "127.0.0.1:7717");
  assert!(request.authorization.is_none());
  assert!(request.last_event_id.is_none());
  assert!(!request.close);
}

/// What the parser made of one request.
struct Parsed(Result<Option<super::Request>, super::ReadError>);

fn parse(bytes: &'static [u8]) -> Parsed {
  let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
  let addr = listener.local_addr().expect("local address");
  let sender = std::thread::spawn(move || {
    let mut stream = TcpStream::connect(addr).expect("connect");
    stream.write_all(bytes).expect("send request");
  });
  let (stream, _) = listener.accept().expect("accept");
  sender.join().expect("sender");
  Parsed(read_request(&mut BufReader::new(stream)))
}

#[test]
fn a_post_carries_its_body_origin_and_content_type() {
  let request = parse(
    b"POST /api/v1/actions/pause HTTP/1.1\r\nHost: 127.0.0.1:7717\r\nOrigin: http://127.0.0.1:7717\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n{}",
  )
  .0
  .expect("parse")
  .expect("request");
  assert_eq!(request.body, b"{}");
  assert_eq!(request.origin.as_deref(), Some("http://127.0.0.1:7717"));
  assert_eq!(request.content_type.as_deref(), Some("application/json"));
}

#[test]
fn body_limits_refuse_what_the_router_must_not_read() {
  let missing = parse(b"POST /api/v1/actions/pause HTTP/1.1\r\nHost: h\r\n\r\n").0;
  assert!(matches!(missing, Err(super::ReadError::Bad)));
  let large = parse(b"POST /a HTTP/1.1\r\nHost: h\r\nContent-Length: 16385\r\n\r\n").0;
  assert!(matches!(large, Err(super::ReadError::TooLarge)));
  let get = parse(b"GET /a HTTP/1.1\r\nHost: h\r\nContent-Length: 1\r\n\r\nx").0;
  assert!(matches!(get, Err(super::ReadError::TooLarge)));
  let chunked = parse(b"POST /a HTTP/1.1\r\nHost: h\r\nTransfer-Encoding: chunked\r\n\r\n").0;
  assert!(matches!(chunked, Err(super::ReadError::Bad)));
  let twice =
    parse(b"POST /a HTTP/1.1\r\nHost: h\r\nOrigin: a\r\nOrigin: b\r\nContent-Length: 0\r\n\r\n").0;
  assert!(matches!(twice, Err(super::ReadError::Bad)));
  let short = parse(b"POST /a HTTP/1.1\r\nHost: h\r\nContent-Length: 5\r\n\r\nab").0;
  assert!(matches!(short, Err(super::ReadError::Io)));
  let empty_get = parse(b"GET /a HTTP/1.1\r\nHost: h\r\nContent-Length: 0\r\n\r\n").0;
  assert_eq!(empty_get.expect("parse").expect("request").body, b"");
}
