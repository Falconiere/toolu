use std::io::{Read as _, Write as _};
use std::net::{SocketAddr, TcpStream};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use serde_json::Value;
use toolu_runtime::config::secrets::Secrets;

use super::Screen;
use crate::fleet_rig::{config, listener, secrets_of};
use crate::herdr_fixture::HerdrFixture;
use crate::herdr_state::Scripted;
use crate::http::{Config, Running, start};
use crate::http_live::{Herdr, Limiter};
use crate::model::{Issue, World};
use crate::paths::Paths;

pub(super) const TOKEN: &str = "SCREEN-TEST-TOKEN";
const PATH: &str = "/api/v1/issues/worker/screen";

pub(super) fn get(addr: SocketAddr, path: &str, token: Option<&str>) -> (u16, Value, usize) {
  let mut stream = TcpStream::connect(addr).expect("connect");
  let auth = token.map_or_else(String::new, |token| {
    format!("Authorization: Bearer {token}\r\n")
  });
  write!(
    stream,
    "GET {path} HTTP/1.1\r\nHost: {addr}\r\n{auth}Connection: close\r\n\r\n"
  )
  .expect("request");
  let mut response = String::new();
  stream.read_to_string(&mut response).expect("response");
  let (headers, body) = response.split_once("\r\n\r\n").expect("HTTP response");
  let status = headers
    .get(9..12)
    .and_then(|value| value.parse().ok())
    .expect("status");
  (
    status,
    serde_json::from_str(body).expect("JSON body"),
    body.len(),
  )
}

fn assert_head(addr: SocketAddr) {
  for (token, expected) in [(None, 401), (Some(TOKEN), 200)] {
    let mut stream = TcpStream::connect(addr).expect("HEAD connect");
    let auth = token.map_or_else(String::new, |token| {
      format!("Authorization: Bearer {token}\r\n")
    });
    write!(
      stream,
      "HEAD {PATH} HTTP/1.1\r\nHost: {addr}\r\n{auth}Connection: close\r\n\r\n"
    )
    .expect("HEAD request");
    let mut response = String::new();
    stream.read_to_string(&mut response).expect("HEAD response");
    let (headers, body) = response.split_once("\r\n\r\n").expect("HEAD headers");
    assert!(headers.starts_with(&format!("HTTP/1.1 {expected} ")));
    assert!(body.is_empty(), "HEAD must have no body");
  }
}

pub(super) fn world_with(key: &str, status: &str) -> World {
  let mut world = World::new(0);
  let mut issue = Issue::blank(key, "epic", "/tmp/epic", 0);
  issue.agent_status = status.to_owned();
  world.issues.insert(issue.key.clone(), issue);
  world
}

pub(super) fn serve(config: Config, world: World) -> (Running, Arc<AtomicBool>, tempfile::TempDir) {
  let root = tempfile::tempdir().expect("root");
  let stop = Arc::new(AtomicBool::new(false));
  let running = start(
    config,
    Paths::at(root.path()),
    Arc::new(Mutex::new(world)),
    Arc::new(Condvar::new()),
    Arc::clone(&stop),
  )
  .expect("listener");
  (running, stop, root)
}

#[test]
fn visible_read_needs_the_token_even_on_loopback() {
  let fixture = HerdrFixture::start();
  let repo = tempfile::tempdir().expect("repo");
  let pane = format!("{}:p1", fixture.add_repo(repo.path()));
  fixture
    .state()
    .start(&serde_json::json!({"name": "worker", "kind": "claude", "pane_id": pane}))
    .expect("agent");
  fixture.state().tail = "CURRENT-VISIBLE-SCREEN".to_owned();
  let secrets = secrets_of(repo.path(), &[("TOOLU_EPIC_STATUS_TOKEN", TOKEN)]);
  let mut config = Config::new("127.0.0.1:0".parse().expect("address"), secrets, Vec::new());
  config.herdr = Herdr::Socket(fixture.socket.clone());
  let (running, stop, _root) = serve(config, world_with("worker", "working"));
  let (denied, _, _) = get(running.addr, PATH, None);
  assert_eq!(denied, 401);
  let (wrong, _, _) = get(running.addr, PATH, Some("wrong"));
  assert_eq!(wrong, 401);
  assert!(
    fixture.calls().is_empty(),
    "unauthorized request reached herdr"
  );
  let (allowed, screen, _) = get(running.addr, PATH, Some(TOKEN));
  assert_eq!(allowed, 200, "{screen}");
  assert_eq!(screen["text"], "CURRENT-VISIBLE-SCREEN");
  assert_eq!(screen["agent_status"], "working");
  let reads: Vec<_> = fixture
    .calls()
    .into_iter()
    .filter(|(method, _)| method == "agent.read")
    .collect();
  assert_eq!(reads.len(), 1);
  assert_eq!(reads[0].1["source"], "visible");
  assert_eq!(reads[0].1["format"], "text");
  assert!(reads[0].1.get("lines").is_none());
  assert_head(running.addr);
  running.stop(&stop).expect("listener thread");
}

#[test]
fn screen_caches_one_second_and_redacts_before_capping_json_bytes() {
  let fixture = HerdrFixture::start();
  fixture.state().tail = format!("{}\n{TOKEN} TAIL-Ω", "x".repeat(70_000));
  let root = tempfile::tempdir().expect("root");
  let secrets = secrets_of(root.path(), &[("TOOLU_EPIC_STATUS_TOKEN", TOKEN)]);
  let mut config = config(0, Vec::new(), secrets);
  config.herdr = Herdr::Socket(fixture.socket.clone());
  let mut world = world_with("worker", "idle");
  let other = Issue::blank("other", "epic", "/tmp/epic", 0);
  world.issues.insert(other.key.clone(), other);
  let (running, stop, _root) = serve(config, world);
  let (first_status, first, length) = get(running.addr, PATH, Some(TOKEN));
  let (second_status, second, _) = get(running.addr, PATH, Some(TOKEN));
  assert_eq!((first_status, second_status), (200, 200));
  assert_eq!(first["sampled_at"], second["sampled_at"]);
  assert!(length <= 65_536, "body length {length}");
  let text = first["text"].as_str().expect("screen text");
  assert!(text.ends_with("<redacted> TAIL-Ω"), "tail was cut");
  assert!(!text.contains(TOKEN));
  assert!(text.len() < 70_000, "top was not trimmed");
  assert_eq!(first["lines"], text.lines().count());
  assert_eq!(fixture.calls().len(), 1, "one read in cache window");
  let (other_status, _, _) = get(running.addr, "/api/v1/issues/other/screen", Some(TOKEN));
  assert_eq!(other_status, 200);
  assert_eq!(fixture.calls().len(), 2, "each key has its own sample");
  fixture.state().tail = "NEW-SCREEN".to_owned();
  std::thread::sleep(Duration::from_millis(1_100));
  let (_, after, _) = get(running.addr, PATH, Some(TOKEN));
  assert_eq!(after["text"], "NEW-SCREEN");
  assert_ne!(after["sampled_at"], first["sampled_at"]);
  assert_eq!(fixture.calls().len(), 3);
  running.stop(&stop).expect("listener thread");
}

#[test]
fn blocked_worker_screen_includes_visible_dialog_and_named_attach_command() {
  let fixture = HerdrFixture::start();
  fixture.state().tail = "Allow this repository? [y/N]".to_owned();
  let root = tempfile::tempdir().expect("root");
  let secrets = secrets_of(root.path(), &[("TOOLU_EPIC_STATUS_TOKEN", TOKEN)]);
  let mut config = config(0, Vec::new(), secrets);
  config.herdr = Herdr::Socket(fixture.socket.clone());
  config.attach_session = Some("probe session".to_owned());
  let (running, stop, _root) = serve(config, world_with("worker", "blocked"));
  let (status, body, _) = get(running.addr, PATH, Some(TOKEN));
  assert_eq!(status, 200, "{body}");
  assert_eq!(body["text"], "Allow this repository? [y/N]");
  assert_eq!(body["agent_status"], "blocked");
  assert_eq!(
    body["attach"],
    "herdr --session 'probe session' agent attach worker"
  );
  running.stop(&stop).expect("listener thread");
}

#[test]
fn not_running_errors_never_become_server_errors() {
  let fixture = HerdrFixture::start();
  let world = Mutex::new(world_with("worker", "unknown"));
  let limiter = Limiter::default();
  for code in ["agent_not_found", "pane_not_found"] {
    fixture.script(
      "agent.read",
      Scripted::Error {
        code: code.to_owned(),
        message: "gone".to_owned(),
      },
    );
    let screen = Screen::new(Herdr::Socket(fixture.socket.clone()), None);
    let response = screen.response("worker", &limiter, &world, &Secrets::none());
    let body: Value = serde_json::from_slice(&response.body).expect("JSON");
    assert_eq!(response.status, 200);
    assert_eq!(body["state"], "not_running");
  }
}

#[test]
fn unavailable_sources_and_busy_limit_never_become_server_errors() {
  let fixture = HerdrFixture::start();
  let world = Mutex::new(world_with("worker", "unknown"));
  let limiter = Limiter::default();
  let missing = tempfile::tempdir().expect("missing socket");
  for (source, reason) in [
    (Herdr::Off, "off"),
    (Herdr::Scripted, "scripted"),
    (
      Herdr::Socket(missing.path().join("absent.sock")),
      "not running",
    ),
  ] {
    let screen = Screen::new(source, None);
    let response = screen.response("worker", &limiter, &world, &Secrets::none());
    let body: Value = serde_json::from_slice(&response.body).expect("JSON");
    assert_eq!(response.status, 200);
    assert_eq!(body["available"], false);
    assert_eq!(body["reason"], reason);
  }
  fixture.state().protocol = 23;
  let mismatch = Screen::new(Herdr::Socket(fixture.socket.clone()), None).response(
    "worker",
    &limiter,
    &world,
    &Secrets::none(),
  );
  let body: Value = serde_json::from_slice(&mismatch.body).expect("JSON");
  assert_eq!(mismatch.status, 200);
  assert_eq!(body["reason"], "protocol 23");
  let screen = Screen::new(Herdr::Socket(fixture.socket.clone()), None);
  let held: Vec<_> = (0..8).filter_map(|_| limiter.try_acquire()).collect();
  let busy = screen.response("worker", &limiter, &world, &Secrets::none());
  assert_eq!(busy.status, 503);
  assert_eq!(
    serde_json::from_slice::<Value>(&busy.body).expect("JSON")["error"],
    "herdr_busy"
  );
  drop(held);
  assert_eq!(
    screen
      .response("unknown", &limiter, &world, &Secrets::none())
      .status,
    404
  );
}

#[test]
fn screen_refuses_no_token_configuration_and_peer_proxy() {
  let root = tempfile::tempdir().expect("root");
  let (running, stop, _root) = serve(
    config(0, Vec::new(), Secrets::none()),
    world_with("worker", "idle"),
  );
  let (status, body, _) = get(running.addr, PATH, None);
  assert_eq!(status, 403);
  assert_eq!(body["error"], "control_disabled");
  running.stop(&stop).expect("listener thread");
  let peer = crate::peer_fixture::peer("box", "127.0.0.1:9".parse().expect("peer addr"));
  let (running, stop) = listener(root.path(), config(0, vec![peer], Secrets::none()));
  let (status, body, _) = get(
    running.addr,
    "/api/v1/fleet/peers/box/issues/worker/screen",
    None,
  );
  assert_eq!(status, 404);
  assert_eq!(body["error"], "not_found");
  running.stop(&stop).expect("listener thread");
}
