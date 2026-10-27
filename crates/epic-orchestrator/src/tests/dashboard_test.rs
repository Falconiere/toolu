use std::io::{Read as _, Write as _};
use std::net::TcpStream;
use std::path::Path;

use toolu_runtime::config::secrets::Secrets;

use crate::fleet_rig::{config, listener};

const ASSET_CACHE: &str = "public, max-age=31536000, immutable";

fn request(addr: std::net::SocketAddr, method: &str, path: &str) -> (String, Vec<u8>) {
  let mut stream = TcpStream::connect(addr).expect("connect");
  write!(
    stream,
    "{method} {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n"
  )
  .expect("request");
  let mut response = Vec::new();
  stream.read_to_end(&mut response).expect("response");
  let split = response
    .windows(4)
    .position(|bytes| bytes == b"\r\n\r\n")
    .expect("header end");
  let head = String::from_utf8(response[..split].to_vec()).expect("headers");
  (head, response[split + 4..].to_vec())
}

#[test]
fn dashboard_asset_routes_match_embedded_bytes_and_security_headers() {
  let temp = tempfile::tempdir().expect("temp root");
  let (running, stop) = listener(temp.path(), config(0, Vec::new(), Secrets::none()));
  let version = env!("CARGO_PKG_VERSION");
  let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/dashboard");
  for (route, file, kind, cache) in [
    ("/", "index.html", "text/html", "no-store"),
    ("/tokens.css", "tokens.css", "text/css", ASSET_CACHE),
    ("/board.css", "board.css", "text/css", ASSET_CACHE),
    ("/js/app.js", "js/app.js", "text/javascript", ASSET_CACHE),
    ("/js/api.js", "js/api.js", "text/javascript", ASSET_CACHE),
    (
      "/js/logic/status.js",
      "js/logic/status.js",
      "text/javascript",
      ASSET_CACHE,
    ),
    (
      "/js/view/dashboard.js",
      "js/view/dashboard.js",
      "text/javascript",
      ASSET_CACHE,
    ),
    (
      "/fonts/archivo.woff2",
      "fonts/archivo.woff2",
      "font/woff2",
      ASSET_CACHE,
    ),
    (
      "/fonts/jetbrains-mono.woff2",
      "fonts/jetbrains-mono.woff2",
      "font/woff2",
      ASSET_CACHE,
    ),
  ] {
    let path = if route == "/" {
      route.to_owned()
    } else {
      format!("/assets/{version}{route}")
    };
    let (headers, body) = request(running.addr, "GET", &path);
    assert!(headers.starts_with("HTTP/1.1 200 OK"), "{path}: {headers}");
    assert_eq!(
      body,
      std::fs::read(assets.join(file)).expect(file),
      "{path}"
    );
    assert!(headers.contains(&format!("Content-Type: {kind}")), "{path}");
    assert!(
      headers.contains(&format!("Cache-Control: {cache}")),
      "{path}"
    );
    assert!(
      headers.contains("X-Content-Type-Options: nosniff"),
      "{path}"
    );
    assert!(headers.contains("Content-Security-Policy: default-src 'self'; script-src 'self'; style-src 'self'; font-src 'self'; img-src 'self' data:; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'none'"), "{path}");
  }
  let (headers, _) = request(
    running.addr,
    "GET",
    &format!("/assets/{version}/fonts/OFL.txt"),
  );
  assert!(headers.starts_with("HTTP/1.1 404"));
  for path in [
    "/",
    &format!("/assets/{version}/tokens.css"),
    "/api/v1/events/stream",
  ] {
    let (headers, body) = request(running.addr, "HEAD", path);
    assert!(headers.starts_with("HTTP/1.1 200"), "{path}: {headers}");
    assert!(body.is_empty(), "{path}");
    assert!(
      headers.contains("Content-Security-Policy: default-src 'self'"),
      "{path}"
    );
    assert!(
      headers.contains("X-Content-Type-Options: nosniff"),
      "{path}"
    );
  }
  let (headers, _) = request(running.addr, "GET", "/api/v1/overview");
  assert!(headers.contains("Cache-Control: no-store"));
  assert!(headers.contains("Content-Security-Policy: default-src 'self'"));
  running.stop(&stop).expect("listener thread");
}

#[test]
fn dashboard_asset_page_is_offline_and_font_licensed() {
  let page = std::fs::read_to_string(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/dashboard/index.html"
  ))
  .expect("page");
  let version = env!("CARGO_PKG_VERSION");
  assert!(page.contains(&format!("/assets/{version}/tokens.css")));
  assert!(page.contains(&format!("/assets/{version}/js/app.js")));
  for forbidden in ["<style", "<script>", "http://", "https://"] {
    assert!(!page.contains(forbidden), "{forbidden}");
  }
  let fonts = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/dashboard/fonts");
  let bytes = ["archivo.woff2", "jetbrains-mono.woff2"]
    .iter()
    .map(|file| std::fs::metadata(fonts.join(file)).expect(file).len())
    .sum::<u64>();
  assert!(bytes < 200_000, "font bytes: {bytes}");
  for license in ["OFL-Archivo.txt", "OFL-JetBrainsMono.txt"] {
    assert!(fonts.join(license).is_file(), "{license}");
  }
}
