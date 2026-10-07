//! Real loopback HTTPS and CONNECT proxy for Rust transport tests.
//! Each fixture owns ephemeral ports and a generated certificate.

use std::collections::BTreeMap;
use std::fmt;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
mod proxy;
mod reply;
mod routes;
mod server;

use proxy::spawn_proxy;
pub use reply::Reply;
use routes::Routes;
use server::{bind, spawn_https, tls_server_config};

/// A fixture setup failure.
#[derive(Debug)]
pub struct Error(String);

impl fmt::Display for Error {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(&self.0)
  }
}

impl std::error::Error for Error {}

fn error(value: impl fmt::Display) -> Error {
  Error(value.to_string())
}

/// One HTTP request observed after the TLS handshake.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservedRequest {
  /// HTTP method.
  pub method: String,
  /// Request path, including any query.
  pub path: String,
  /// Lowercase header names and their values.
  pub headers: BTreeMap<String, String>,
  /// Request body bytes.
  pub body: Vec<u8>,
}

/// Shared HTTPS server state for the two fixture origins.
pub(crate) struct ServerData {
  pub(crate) routes: Arc<Mutex<Routes>>,
  pub(crate) requests: Arc<Mutex<Vec<ObservedRequest>>>,
  pub(crate) diagnostics: Arc<Mutex<Vec<String>>>,
}

impl ServerData {
  fn new(
    routes: &Arc<Mutex<Routes>>,
    requests: &Arc<Mutex<Vec<ObservedRequest>>>,
    diagnostics: &Arc<Mutex<Vec<String>>>,
  ) -> Arc<Self> {
    Arc::new(Self {
      routes: Arc::clone(routes),
      requests: Arc::clone(requests),
      diagnostics: Arc::clone(diagnostics),
    })
  }
}

/// Two HTTPS origins with one shared route table and a CONNECT proxy.
pub struct Fixture {
  cert_der: Vec<u8>,
  first_origin: SocketAddr,
  second_origin: SocketAddr,
  proxy: SocketAddr,
  routes: Arc<Mutex<Routes>>,
  requests: Arc<Mutex<Vec<ObservedRequest>>>,
  connects: Arc<Mutex<Vec<String>>>,
  diagnostics: Arc<Mutex<Vec<String>>>,
  stop: Arc<AtomicBool>,
  handles: Vec<JoinHandle<()>>,
}

impl Fixture {
  /// Start two TLS origins and a CONNECT proxy on ephemeral loopback ports.
  ///
  /// # Errors
  /// Returns a setup error if certificate generation, TLS configuration, or
  /// loopback binding fails.
  pub fn start() -> Result<Self, Error> {
    let (cert_der, config) = tls_server_config()?;
    let routes = Arc::new(Mutex::new(Routes::default()));
    let requests = Arc::new(Mutex::new(Vec::new()));
    let connects = Arc::new(Mutex::new(Vec::new()));
    let diagnostics = Arc::new(Mutex::new(Vec::new()));
    let server_data = ServerData::new(&routes, &requests, &diagnostics);
    let stop = Arc::new(AtomicBool::new(false));
    let first = bind()?;
    let second = bind()?;
    let proxy_listener = bind()?;
    let first_origin = first.local_addr().map_err(error)?;
    let second_origin = second.local_addr().map_err(error)?;
    let proxy = proxy_listener.local_addr().map_err(error)?;
    let handles = vec![
      spawn_https(
        first,
        Arc::clone(&config),
        Arc::clone(&server_data),
        Arc::clone(&stop),
      ),
      spawn_https(second, config, server_data, Arc::clone(&stop)),
      spawn_proxy(
        proxy_listener,
        [first_origin, second_origin],
        Arc::clone(&connects),
        Arc::clone(&diagnostics),
        Arc::clone(&stop),
      ),
    ];
    Ok(Self {
      cert_der,
      first_origin,
      second_origin,
      proxy,
      routes,
      requests,
      connects,
      diagnostics,
      stop,
      handles,
    })
  }

  /// Certificate DER to supply as the client's test root.
  pub fn root_ca_der(&self) -> &[u8] {
    &self.cert_der
  }

  /// URL under the first TLS origin.
  pub fn url(&self, path: &str) -> String {
    format!(
      "https://api.example.test:{}{path}",
      self.first_origin.port()
    )
  }

  /// URL under a second origin with the same hostname but a different port.
  pub fn second_url(&self, path: &str) -> String {
    format!(
      "https://api.example.test:{}{path}",
      self.second_origin.port()
    )
  }

  /// HTTP CONNECT proxy URL.
  pub fn proxy_url(&self) -> String {
    format!("http://{}", self.proxy)
  }

  /// Add or replace an exact request-path response.
  ///
  /// # Errors
  /// Returns an error if the route table lock is poisoned.
  pub fn route(&self, path: &str, reply: Reply) -> Result<(), Error> {
    self.sequence(path, vec![reply])
  }

  /// Answer `path`'s requests with `replies` in order; the last one repeats
  /// once the others are used. An empty sequence removes the route.
  ///
  /// # Errors
  /// Returns an error if the route table lock is poisoned.
  pub fn sequence(&self, path: &str, replies: Vec<Reply>) -> Result<(), Error> {
    self.routes.lock().map_err(error)?.set(path, replies);
    Ok(())
  }

  /// Requests observed after TLS decryption.
  ///
  /// # Errors
  /// Returns an error if the request log lock is poisoned.
  pub fn requests(&self) -> Result<Vec<ObservedRequest>, Error> {
    Ok(self.requests.lock().map_err(error)?.clone())
  }

  /// CONNECT authorities observed by the proxy.
  ///
  /// # Errors
  /// Returns an error if the CONNECT log lock is poisoned.
  pub fn connects(&self) -> Result<Vec<String>, Error> {
    Ok(self.connects.lock().map_err(error)?.clone())
  }

  /// Worker progress and failures, to diagnose a failed transport test.
  ///
  /// # Errors
  /// Returns an error if the diagnostic log lock is poisoned.
  pub fn diagnostics(&self) -> Result<Vec<String>, Error> {
    Ok(self.diagnostics.lock().map_err(error)?.clone())
  }
}

impl Drop for Fixture {
  fn drop(&mut self) {
    self.stop.store(true, Ordering::SeqCst);
    for handle in self.handles.drain(..) {
      let _ = handle.join();
    }
  }
}

#[cfg(test)]
#[path = "tests/lib_test.rs"]
mod tests;
