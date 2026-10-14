//! A loopback HTTP peer for fleet tests: one canned reply per request, and
//! every request head recorded so a test can see what the peer received.

use std::io::{BufRead as _, BufReader, Write as _};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use toolu_runtime::config::epic::Peer;

/// A running peer and the request heads it has read.
pub(crate) struct PeerServer {
  pub addr: SocketAddr,
  pub heads: Arc<Mutex<Vec<String>>>,
}

impl PeerServer {
  /// This server as the config peer `name`.
  pub(crate) fn peer(&self, name: &str) -> Peer {
    peer(name, self.addr)
  }

  /// How many `GET /api/v1/overview` requests arrived.
  pub(crate) fn overview_requests(&self) -> usize {
    let heads = self.heads.lock().expect("heads");
    heads
      .iter()
      .filter(|head| head.starts_with("GET /api/v1/overview "))
      .count()
  }

  /// Every request head received so far, joined.
  pub(crate) fn seen(&self) -> String {
    self.heads.lock().expect("heads").join("\n")
  }
}

/// The config peer `name` at `addr`, with the origin form `epic::parse` gives.
pub(crate) fn peer(name: &str, addr: SocketAddr) -> Peer {
  Peer {
    name: name.to_owned(),
    url: format!("http://{addr}/"),
  }
}

/// Answer every request with `status`, `content_type` and `body`.
pub(crate) fn serve(status: u16, content_type: &str, body: &str) -> PeerServer {
  serve_after(Duration::ZERO, status, content_type, body)
}

/// As [`serve`], each reply sent `delay` after its request head arrived.
pub(crate) fn serve_after(
  delay: Duration,
  status: u16,
  content_type: &str,
  body: &str,
) -> PeerServer {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind peer");
  let addr = listener.local_addr().expect("peer address");
  let heads = Arc::new(Mutex::new(Vec::new()));
  let seen = Arc::clone(&heads);
  let body = body.to_owned();
  let content_type = content_type.to_owned();
  thread::spawn(move || {
    for stream in listener.incoming() {
      let Ok(mut stream) = stream else { return };
      let head = read_head(&stream);
      seen.lock().expect("heads").push(head);
      thread::sleep(delay);
      let reply = format!(
        "HTTP/1.1 {status} X\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
      );
      let _sent = stream.write_all(reply.as_bytes());
    }
  });
  PeerServer { addr, heads }
}

fn read_head(stream: &TcpStream) -> String {
  let mut reader = BufReader::new(stream.try_clone().expect("clone peer stream"));
  let mut head = String::new();
  let mut line = String::new();
  while reader.read_line(&mut line).unwrap_or(0) > 0 && line != "\r\n" {
    head.push_str(&line);
    line.clear();
  }
  head
}

/// A listener that never accepts: the connect succeeds and no reply comes.
pub(crate) fn blackhole() -> TcpListener {
  TcpListener::bind("127.0.0.1:0").expect("bind blackhole")
}

/// A loopback address with nothing listening.
pub(crate) fn closed() -> SocketAddr {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind closed");
  listener.local_addr().expect("closed address")
}
