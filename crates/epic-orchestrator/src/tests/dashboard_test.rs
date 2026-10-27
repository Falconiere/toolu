use std::io::{Read as _, Write as _};
use std::net::TcpStream;
use std::path::Path;

use serde_json::json;
use toolu_runtime::config::secrets::Secrets;

use crate::disk::write_value;
use crate::fleet_rig::{Engine, config, free_port, get_json, listener};

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

#[test]
fn dashboard_status_capture_comes_from_running_engines() {
  let temp = tempfile::tempdir().expect("temp root");
  let peer = Engine::start_titled(
    &temp.path().join("peer"),
    ("peer-649", "Peer dashboard"),
    "peer-658",
    config(free_port(), Vec::new(), Secrets::none()),
  );
  let local = Engine::start_titled(
    &temp.path().join("local"),
    ("local-649", "Dashboard redesign"),
    "local-658",
    config(free_port(), vec![peer.peer("build-mini")], Secrets::none()),
  );
  let issue_dir = local.root.join("local-649/issues");
  std::fs::create_dir_all(&issue_dir).expect("issue dir");
  write_value(
    &issue_dir.join("local-658.json"),
    &json!({
      "key": "local-658",
      "kind": "claude",
      "worktree": Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }),
  )
  .expect("issue record");
  let captured = json!({
    "overview": get_json(local.addr, "/api/v1/overview"),
    "epics": get_json(local.addr, "/api/v1/epics"),
    "board": get_json(local.addr, "/api/v1/epics/local-649"),
    "hosts": get_json(local.addr, "/api/v1/hosts"),
    "attention": get_json(local.addr, "/api/v1/attention"),
    "fleet": get_json(local.addr, "/api/v1/fleet"),
    "events": get_json(local.addr, "/api/v1/events?limit=7"),
    "live": get_json(local.addr, "/api/v1/agents/live"),
    "peer_overview": get_json(local.addr, "/api/v1/fleet/peers/build-mini/overview"),
    "peer_board": get_json(local.addr, "/api/v1/fleet/peers/build-mini/epics/peer-649")
  });
  assert_eq!(captured["board"]["issues"][0]["key"], "local-658");
  assert_eq!(captured["peer_board"]["issues"][0]["key"], "peer-658");
  assert_eq!(captured["fleet"]["machines"][1]["name"], "build-mini");
  if std::env::var_os("TOOLU_CAPTURE_DASHBOARD").is_some() {
    let path =
      Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/epic/dashboard/status.json");
    std::fs::create_dir_all(path.parent().expect("fixture parent")).expect("fixture dir");
    std::fs::write(
      &path,
      serde_json::to_vec_pretty(&captured).expect("fixture json"),
    )
    .expect("write fixture");
  }
  local.stop();
  peer.stop();
}
