//! Loopback TLS server for the shared HTTP fixture.

use std::collections::BTreeMap;
use std::io::{self, BufRead, BufReader, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use toolu_http::fixture_rustls as rustls;

use crate::{Error, ObservedRequest, Reply, error};

/// Generate a self-signed test certificate and rustls server configuration.
///
/// # Errors
/// Returns an error if certificate generation or TLS setup fails.
pub(crate) fn tls_server_config() -> Result<(Vec<u8>, Arc<rustls::ServerConfig>), Error> {
  let key = rcgen::generate_simple_self_signed(vec!["api.example.test".into()]).map_err(error)?;
  let cert_der = key.cert.der().to_vec();
  let private = rustls::pki_types::PrivatePkcs8KeyDer::from(key.signing_key.serialize_der());
  let config =
    rustls::ServerConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
      .with_safe_default_protocol_versions()
      .map_err(error)?
      .with_no_client_auth()
      .with_single_cert(
        vec![rustls::pki_types::CertificateDer::from(cert_der.clone())],
        private.into(),
      )
      .map_err(error)?;
  Ok((cert_der, Arc::new(config)))
}

/// Bind a nonblocking ephemeral loopback listener.
///
/// # Errors
/// Returns an I/O error when binding or configuring the listener fails.
pub(crate) fn bind() -> Result<TcpListener, Error> {
  let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).map_err(error)?;
  listener.set_nonblocking(true).map_err(error)?;
  Ok(listener)
}

/// Accept real TLS connections until `stop` is set.
pub(crate) fn spawn_https(
  listener: TcpListener,
  config: Arc<rustls::ServerConfig>,
  routes: Arc<Mutex<BTreeMap<String, Reply>>>,
  requests: Arc<Mutex<Vec<ObservedRequest>>>,
  stop: Arc<AtomicBool>,
) -> JoinHandle<()> {
  thread::spawn(move || {
    while !stop.load(Ordering::SeqCst) {
      match listener.accept() {
        Ok((socket, _)) => spawn_connection(socket, &config, &routes, &requests),
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
  config: &Arc<rustls::ServerConfig>,
  routes: &Arc<Mutex<BTreeMap<String, Reply>>>,
  requests: &Arc<Mutex<Vec<ObservedRequest>>>,
) {
  let config = Arc::clone(config);
  let routes = Arc::clone(routes);
  let requests = Arc::clone(requests);
  thread::spawn(move || {
    let _ = serve_https(socket, config, &routes, &requests);
  });
}

fn serve_https(
  socket: TcpStream,
  config: Arc<rustls::ServerConfig>,
  routes: &Arc<Mutex<BTreeMap<String, Reply>>>,
  requests: &Arc<Mutex<Vec<ObservedRequest>>>,
) -> Result<(), Error> {
  socket
    .set_read_timeout(Some(Duration::from_secs(2)))
    .map_err(error)?;
  let connection = rustls::ServerConnection::new(config).map_err(error)?;
  let stream = rustls::StreamOwned::new(connection, socket);
  let mut reader = BufReader::new(stream);
  let request = read_request(&mut reader)?;
  let reply = routes
    .lock()
    .map_err(error)?
    .get(&request.path)
    .cloned()
    .unwrap_or_else(|| Reply::new(404, "missing route"));
  requests.lock().map_err(error)?.push(request);
  if !reply.delay.is_zero() {
    thread::sleep(reply.delay);
  }
  write_reply(reader.get_mut(), &reply).map_err(error)
}

fn read_request<R: BufRead>(reader: &mut R) -> Result<ObservedRequest, Error> {
  let mut first = String::new();
  reader.read_line(&mut first).map_err(error)?;
  let mut parts = first.split_whitespace();
  let method = parts
    .next()
    .ok_or_else(|| error("missing method"))?
    .to_owned();
  let path = parts
    .next()
    .ok_or_else(|| error("missing path"))?
    .to_owned();
  let mut headers = BTreeMap::new();
  loop {
    let mut line = String::new();
    reader.read_line(&mut line).map_err(error)?;
    if line == "\r\n" || line.is_empty() {
      break;
    }
    let (name, value) = line.split_once(':').ok_or_else(|| error("bad header"))?;
    headers.insert(name.to_ascii_lowercase(), value.trim().to_owned());
  }
  let length = headers
    .get("content-length")
    .and_then(|value| value.parse::<usize>().ok())
    .unwrap_or(0);
  if length > 1024 * 1024 {
    return Err(error("request too large"));
  }
  let mut body = vec![0; length];
  reader.read_exact(&mut body).map_err(error)?;
  Ok(ObservedRequest {
    method,
    path,
    headers,
    body,
  })
}

fn write_reply<W: Write>(writer: &mut W, reply: &Reply) -> io::Result<()> {
  let reason = match reply.status {
    200 => "OK",
    302 => "Found",
    404 => "Not Found",
    500 => "Internal Server Error",
    _ => "Response",
  };
  write!(
    writer,
    "HTTP/1.1 {} {}\r\nContent-Length: {}\r\nConnection: close\r\n",
    reply.status,
    reason,
    reply.body.len()
  )?;
  for (name, value) in &reply.headers {
    write!(writer, "{name}: {value}\r\n")?;
  }
  writer.write_all(b"\r\n")?;
  writer.write_all(&reply.body)?;
  writer.flush()
}

#[cfg(test)]
#[path = "tests/server_test.rs"]
mod tests;
