use std::io::Write;
use std::net::TcpStream;

use crate::Fixture;

#[test]
fn proxy_rejects_a_plain_get_request() {
  let fixture = Fixture::start().expect("fixture");
  let address = fixture.proxy_url().trim_start_matches("http://").to_owned();
  let mut socket = TcpStream::connect(address).expect("proxy socket");
  socket
    .write_all(b"GET / HTTP/1.1\r\n\r\n")
    .expect("send request");
  drop(socket);
  assert_eq!(fixture.connects().expect("connects").len(), 0);
}
