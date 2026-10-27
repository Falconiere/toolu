use std::io::{Read as _, Write as _};
use std::net::TcpStream;
use std::path::Path;

use serde_json::json;
use toolu_runtime::config::secrets::Secrets;
use toolu_state::time::epoch_millis;

use crate::disk::write_value;
use crate::fleet_rig::{Engine, config, free_port, get_json, listener, secrets_of};

const ASSET_CACHE: &str = "public, max-age=31536000, immutable";
const ASSET_ROUTES: [(&str, &str, &str, &str); 12] = [
  ("/", "index.html", "text/html", "no-store"),
  ("/tokens.css", "tokens.css", "text/css", ASSET_CACHE),
  ("/board.css", "board.css", "text/css", ASSET_CACHE),
  (
    "/board-layout.css",
    "board-layout.css",
    "text/css",
    ASSET_CACHE,
  ),
  ("/panels.css", "panels.css", "text/css", ASSET_CACHE),
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
    "/js/view/peer.js",
    "js/view/peer.js",
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
];

fn request(addr: std::net::SocketAddr, method: &str, path: &str) -> (String, Vec<u8>) {
  request_with(addr, method, path, "")
}

fn request_with(
  addr: std::net::SocketAddr,
  method: &str,
  path: &str,
  headers: &str,
) -> (String, Vec<u8>) {
  let mut stream = TcpStream::connect(addr).expect("connect");
  write!(
    stream,
    "{method} {path} HTTP/1.1\r\nHost: {addr}\r\n{headers}Connection: close\r\n\r\n"
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

fn assert_asset(
  addr: std::net::SocketAddr,
  version: &str,
  assets: &Path,
  (route, file, kind, cache): (&str, &str, &str, &str),
) {
  let path = if route == "/" {
    route.to_owned()
  } else {
    format!("/assets/{version}{route}")
  };
  let (headers, body) = request(addr, "GET", &path);
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

#[test]
fn dashboard_asset_routes_match_embedded_bytes_and_security_headers() {
  let temp = tempfile::tempdir().expect("temp root");
  let (running, stop) = listener(temp.path(), config(0, Vec::new(), Secrets::none()));
  let version = env!("CARGO_PKG_VERSION");
  let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/dashboard");
  for spec in ASSET_ROUTES {
    assert_asset(running.addr, version, &assets, spec);
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
fn dashboard_page_has_the_board_panels_and_accessible_states() {
  let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/dashboard");
  let page = std::fs::read_to_string(assets.join("index.html")).expect("page");
  for id in [
    "tile-workers",
    "tile-attention",
    "tile-hosts",
    "tile-plans",
    "board",
    "hosts",
    "attention",
    "journal",
    "peer-banner",
    "peer-link",
  ] {
    assert!(page.contains(&format!("id=\"{id}\"")), "{id}");
  }
  let states = std::fs::read_to_string(assets.join("js/logic/status.js")).expect("states");
  for label in [
    "● WORKING",
    "▲ BLOCKED",
    "/ IDLE",
    "✓ DONE",
    "? UNKNOWN",
    "○ QUEUED",
    "● IN ROTATION",
    "◷ COOLING",
    "‖ PAUSED",
  ] {
    assert!(states.contains(label), "{label}");
  }
  let motion = std::fs::read_to_string(assets.join("panels.css")).expect("motion");
  assert!(motion.contains("@media (prefers-reduced-motion: reduce)"));
  assert!(motion.contains("animation: none"));
  for file in ["tokens.css", "board.css", "board-layout.css", "panels.css"] {
    let css = std::fs::read_to_string(assets.join(file)).expect(file);
    assert!(
      !css.contains("http://") && !css.contains("https://"),
      "{file}"
    );
  }
}

fn save_capture(captured: &serde_json::Value) {
  if std::env::var_os("TOOLU_CAPTURE_DASHBOARD").is_none() {
    return;
  }
  let path =
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/epic/dashboard/status.json");
  std::fs::create_dir_all(path.parent().expect("fixture parent")).expect("fixture dir");
  std::fs::write(
    &path,
    serde_json::to_vec_pretty(captured).expect("fixture json"),
  )
  .expect("write fixture");
}

const TOKEN: &str = "dashboard-capture-token";

/// The status code and JSON body of a worker screen read.
fn screen(addr: std::net::SocketAddr, headers: &str) -> serde_json::Value {
  let (head, body) = request_with(addr, "GET", "/api/v1/issues/local-658/screen", headers);
  let status: u16 = head
    .get(9..12)
    .and_then(|code| code.parse().ok())
    .expect("status code");
  json!({"status": status, "body": serde_json::from_slice::<serde_json::Value>(&body).expect("json")})
}

fn seed_host_states(root: &Path) {
  let now = epoch_millis(std::time::SystemTime::now());
  write_value(
    &root.join("state.json"),
    &json!({
      "version": 1,
      "leases": [],
      "cooldowns": {"codex": {"until": now + 41 * 60_000, "reason": "usage limit"}},
      "paused_hosts": {"muse": {"by": "user", "at": now}}
    }),
  )
  .expect("host states");
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
    config(
      free_port(),
      vec![peer.peer("build-mini")],
      secrets_of(temp.path(), &[("TOOLU_EPIC_STATUS_TOKEN", TOKEN)]),
    ),
  );
  seed_host_states(&local.paths.root);
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
    "peer_board": get_json(local.addr, "/api/v1/fleet/peers/build-mini/epics/peer-649"),
    "issue": get_json(local.addr, "/api/v1/issues/local-658"),
    "peer_issue": get_json(local.addr, "/api/v1/fleet/peers/build-mini/issues/peer-658"),
    "screen": screen(local.addr, &format!("Authorization: Bearer {TOKEN}\r\n")),
    "screen_locked": screen(local.addr, ""),
    "screen_disabled": screen(peer.addr, "")
  });
  assert_eq!(captured["issue"]["key"], "local-658");
  assert_eq!(captured["screen_locked"]["status"], 401);
  assert_eq!(captured["screen_disabled"]["status"], 403);
  assert_eq!(captured["board"]["issues"][0]["key"], "local-658");
  assert_eq!(captured["peer_board"]["issues"][0]["key"], "peer-658");
  assert_eq!(captured["fleet"]["machines"][1]["name"], "build-mini");
  assert_eq!(captured["hosts"]["items"][1]["reason"], "usage limit");
  assert_eq!(captured["hosts"]["items"][3]["paused"], true);
  save_capture(&captured);
  local.stop();
  peer.stop();
}
