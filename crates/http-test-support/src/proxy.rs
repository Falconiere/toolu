//! Real HTTP CONNECT relay to the fixture's TLS origins.

use std::io::{self, BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::{Error, error};

/// Accept HTTP CONNECT tunnels to the registered TLS origins.
pub(crate) fn spawn_proxy(
  listener: TcpListener,
  origins: [SocketAddr; 2],
  connects: Arc<Mutex<Vec<String>>>,
  diagnostics: Arc<Mutex<Vec<String>>>,
  stop: Arc<AtomicBool>,
) -> JoinHandle<()> {
  thread::spawn(move || {
    while !stop.load(Ordering::SeqCst) {
      match listener.accept() {
        Ok((socket, _)) => spawn_connection(socket, origins, &connects, &diagnostics),
        Err(err) if err.kind() == io::ErrorKind::WouldBlock => {
          thread::sleep(Duration::from_millis(2));
        }
        Err(_) => break,
      }
    }
  })
}

fn spawn_connection(
  socket: TcpStream,
  origins: [SocketAddr; 2],
  connects: &Arc<Mutex<Vec<String>>>,
  diagnostics: &Arc<Mutex<Vec<String>>>,
) {
  let connects = Arc::clone(connects);
  let diagnostics = Arc::clone(diagnostics);
  thread::spawn(move || {
    if let Err(err) = serve_proxy(socket, origins, &connects) {
      let _ = diagnostics
        .lock()
        .map(|mut log| log.push(format!("CONNECT: {err}")));
    }
  });
}

fn serve_proxy(
  socket: TcpStream,
  origins: [SocketAddr; 2],
  connects: &Arc<Mutex<Vec<String>>>,
) -> Result<(), Error> {
  socket
    .set_read_timeout(Some(Duration::from_secs(2)))
    .map_err(error)?;
  let mut reader = BufReader::new(socket);
  let mut first = String::new();
  reader.read_line(&mut first).map_err(error)?;
  let mut parts = first.split_whitespace();
  if parts.next() != Some("CONNECT") {
    return Err(error("expected CONNECT"));
  }
  let authority = parts
    .next()
    .ok_or_else(|| error("missing CONNECT authority"))?
    .to_owned();
  loop {
    let mut line = String::new();
    reader.read_line(&mut line).map_err(error)?;
    if line == "\r\n" || line.is_empty() {
      break;
    }
  }
  connects.lock().map_err(error)?.push(authority.clone());
  let (_, text_port) = authority
    .rsplit_once(':')
    .ok_or_else(|| error("missing CONNECT port"))?;
  let port = text_port.parse::<u16>().map_err(error)?;
  let addr = origins
    .into_iter()
    .find(|origin| origin.port() == port)
    .ok_or_else(|| error("unregistered CONNECT port"))?;
  let mut upstream = TcpStream::connect(addr).map_err(error)?;
  let mut client = reader.into_inner();
  client
    .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
    .map_err(error)?;
  let mut client_read = client.try_clone().map_err(error)?;
  let mut upstream_write = upstream.try_clone().map_err(error)?;
  let forward = thread::spawn(move || io::copy(&mut client_read, &mut upstream_write));
  io::copy(&mut upstream, &mut client).map_err(error)?;
  forward
    .join()
    .map_err(|_panic| error("CONNECT copy thread panicked"))?
    .map_err(error)?;
  Ok(())
}

#[cfg(test)]
#[path = "tests/proxy_test.rs"]
mod tests;
