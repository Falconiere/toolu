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
