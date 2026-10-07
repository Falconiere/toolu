use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::Arc;
use toolu_http::{Auth, Client, Config, Error};
use toolu_runtime::env::Env;

use toolu_http::fixture_rustls as rustls;

use crate::Fixture;

#[test]
fn missing_route_returns_real_http_404() {
  let fixture = Fixture::start().expect("fixture");
  let config = Config {
    test_root_ca_der: Some(fixture.root_ca_der().to_vec()),
    ..Config::default()
  };
  let env = Env::from_pairs([("HTTPS_PROXY", fixture.proxy_url())]);
  let client = Client::new(config, &env).expect("client");
  assert!(matches!(
    client.get_bytes(&fixture.url("/missing"), &Auth::None),
    Err(Error::HttpStatus(404))
  ));
}

#[test]
fn tls_server_closes_with_close_notify() {
  let fixture = Fixture::start().expect("fixture");
  let mut roots = rustls::RootCertStore::empty();
  roots
    .add(rustls::pki_types::CertificateDer::from(
      fixture.root_ca_der().to_vec(),
    ))
    .expect("test root");
  let config =
    rustls::ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
      .with_safe_default_protocol_versions()
      .expect("protocol versions")
      .with_root_certificates(roots)
      .with_no_client_auth();
  let connection = rustls::ClientConnection::new(
    Arc::new(config),
    rustls::pki_types::ServerName::try_from("api.example.test").expect("server name"),
  )
  .expect("TLS connection");
  let socket = TcpStream::connect(fixture.first_origin).expect("origin");
  let mut stream = rustls::StreamOwned::new(connection, socket);
  stream
    .write_all(b"GET /missing HTTP/1.1\r\nHost: api.example.test\r\nConnection: close\r\n\r\n")
    .expect("request");
  let mut response = Vec::new();
  stream.read_to_end(&mut response).expect("clean TLS EOF");
  assert!(response.starts_with(b"HTTP/1.1 404 Not Found\r\n"));
}
