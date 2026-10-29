//! Keep-alive accept counts prove retained Agents reuse connections.

use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread;
use std::time::Duration;

use toolu_http::{Auth, Client, Config, Method, Request};
use toolu_runtime::env::Env;

struct KeepAlive {
  addr: String,
  accepts: Arc<AtomicUsize>,
  close_after: Arc<AtomicUsize>,
  stop: Arc<AtomicBool>,
}

impl KeepAlive {
  fn start() -> std::io::Result<Self> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let addr = format!("http://{}", listener.local_addr()?);
    let accepts = Arc::new(AtomicUsize::new(0));
    let close_after = Arc::new(AtomicUsize::new(usize::MAX));
    let stop = Arc::new(AtomicBool::new(false));
    let accepts_h = Arc::clone(&accepts);
    let close_h = Arc::clone(&close_after);
    let stop_h = Arc::clone(&stop);
    listener.set_nonblocking(true)?;
    thread::spawn(move || accept_loop(&listener, &accepts_h, &close_h, &stop_h));
    Ok(Self {
      addr,
      accepts,
      close_after,
      stop,
    })
  }

  fn url(&self, path: &str) -> String {
    format!("{}{path}", self.addr)
  }

  fn accepts(&self) -> usize {
    self.accepts.load(Ordering::SeqCst)
  }
}

impl Drop for KeepAlive {
  fn drop(&mut self) {
    self.stop.store(true, Ordering::SeqCst);
  }
}

fn accept_loop(
  listener: &TcpListener,
  accepts: &AtomicUsize,
  close_after: &Arc<AtomicUsize>,
  stop: &AtomicBool,
) {
  while !stop.load(Ordering::SeqCst) {
    match listener.accept() {
      Ok((stream, _)) => {
        accepts.fetch_add(1, Ordering::SeqCst);
        let close_after = Arc::clone(close_after);
        thread::spawn(move || serve(stream, &close_after));
      }
      Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
        thread::sleep(Duration::from_millis(2));
      }
      Err(_) => break,
    }
  }
}

fn read_headers(stream: &mut TcpStream) -> Option<Vec<u8>> {
  let mut buf = [0_u8; 4096];
  let mut got = Vec::new();
  loop {
    match stream.read(&mut buf) {
      Ok(0) => return None,
      Ok(n) => {
        got.extend_from_slice(buf.get(..n)?);
        if got.windows(4).any(|w| w == b"\r\n\r\n") {
          return Some(got);
        }
      }
      Err(err)
        if matches!(
          err.kind(),
          std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
        ) =>
      {
        if got.is_empty() {
          continue;
        }
        return None;
      }
      Err(_) => return None,
    }
  }
}

fn write_ok(stream: &mut TcpStream, close: bool) -> std::io::Result<()> {
  let connection = if close { "close" } else { "keep-alive" };
  let body = b"ok";
  write!(
    stream,
    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: {connection}\r\n\r\n",
    body.len()
  )?;
  stream.write_all(body)?;
  stream.flush()
}

fn serve(mut stream: TcpStream, close_after: &Arc<AtomicUsize>) {
  let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
  let mut handled = 0_usize;
  while read_headers(&mut stream).is_some() {
    handled += 1;
    let close_now = handled >= close_after.load(Ordering::SeqCst);
    if write_ok(&mut stream, close_now).is_err() {
      return;
    }
    if close_now {
      let _ = stream.shutdown(Shutdown::Both);
      return;
    }
  }
}

fn get(url: &str) -> Request<'_> {
  Request {
    method: Method::Get,
    url,
    auth: &Auth::None,
    headers: &[],
    body: None,
  }
}

#[test]
fn retained_client_reuses_keepalive_connections_and_recovers_after_close() {
  let server = KeepAlive::start().expect("server");
  let client = Client::new(
    Config {
      timeout: Duration::from_secs(2),
      ..Config::default()
    },
    &Env::default(),
  )
  .expect("client");
  let url = server.url("/ok");
  for _ in 0..4 {
    let response = client.send(&get(&url)).expect("send");
    assert_eq!(response.status, 200);
    assert_eq!(response.body, b"ok");
  }
  let accepts = server.accepts();
  assert!(
    accepts < 4,
    "expected connection reuse, accepts={accepts} for 4 requests"
  );

  server.close_after.store(1, Ordering::SeqCst);
  let _ = client.send(&get(&url)).expect("close path");
  let after_close = server.accepts();
  let response = client.send(&get(&url)).expect("recover");
  assert_eq!(response.status, 200);
  assert!(
    server.accepts() > after_close,
    "expected a new accept after the server closed the prior connection"
  );
}

#[test]
fn distinct_clients_do_not_share_connection_pools() {
  let server = KeepAlive::start().expect("server");
  let config = Config {
    timeout: Duration::from_secs(2),
    ..Config::default()
  };
  let first = Client::new(config.clone(), &Env::default()).expect("first");
  let second = Client::new(config, &Env::default()).expect("second");
  let url = server.url("/ok");
  for _ in 0..3 {
    assert_eq!(first.send(&get(&url)).expect("first").status, 200);
  }
  let after_first = server.accepts();
  assert!(
    after_first < 3,
    "first client should reuse, accepts={after_first}"
  );
  for _ in 0..3 {
    assert_eq!(second.send(&get(&url)).expect("second").status, 200);
  }
  let total = server.accepts();
  assert!(
    total > after_first,
    "second client must open its own connection(s); after_first={after_first} total={total}"
  );
}

#[test]
fn with_timeout_rejects_zero_and_shares_the_pool() {
  let server = KeepAlive::start().expect("server");
  let client = Client::new(Config::default(), &Env::default()).expect("client");
  assert!(client.with_timeout(Duration::ZERO).is_err());
  let url = server.url("/ok");
  assert_eq!(client.send(&get(&url)).expect("warm").status, 200);
  let warmed = server.accepts();
  let shorter = client
    .with_timeout(Duration::from_secs(2))
    .expect("with_timeout");
  for _ in 0..3 {
    assert_eq!(shorter.send(&get(&url)).expect("shared").status, 200);
  }
  assert_eq!(
    server.accepts(),
    warmed,
    "with_timeout must reuse the same Agent pool"
  );
}
