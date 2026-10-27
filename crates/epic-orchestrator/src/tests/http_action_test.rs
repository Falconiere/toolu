//! A real engine on loopback, real TCP requests: every refusal stops before
//! the state loop, and the token never leaves the server.

use std::io::{Read as _, Write as _};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use serde_json::json;
use toolu_runtime::config::secrets;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;

use crate::PROTOCOL;
use crate::client::exchange_retry;
use crate::http::Config;
use crate::paths::Paths;
use crate::server::Fault;
use crate::socket::{ServeOptions, serve_with_http};

pub(crate) const TOKEN: &str = "CANARY-TOKEN-650";

pub(crate) struct Live {
  pub paths: Paths,
  pub root: PathBuf,
  pub addr: SocketAddr,
  engine: thread::JoinHandle<Result<(), String>>,
}

impl Live {
  /// Stop the engine and return its result.
  pub(crate) fn stop(self) {
    exchange_retry(&self.paths, PROTOCOL, &json!({"op": "stop"})).expect("stop");
    self.engine.join().expect("engine thread").expect("engine");
  }
}

/// A prompt script that records each body; `SLOW` sleeps 1 s and `MAYBE`
/// reports an uncertain delivery.
fn scripts(root: &Path) -> PathBuf {
  let dir = root.join("scripts");
  std::fs::create_dir_all(&dir).expect("scripts");
  let prompts = root.join("prompts");
  let script = format!(
    "body=$(cat)\nprintf '%s\\n' \"$body\" >> '{}'\ncase \"$body\" in\n*SLOW*) sleep 1; printf '{{\"outcome\":\"sent\"}}' ;;\n*MAYBE*) printf '{{\"outcome\":\"uncertain\"}}' ;;\n*) printf '{{\"outcome\":\"sent\"}}' ;;\nesac\n",
    prompts.display()
  );
  std::fs::write(dir.join("prompt"), script).expect("prompt script");
  dir
}

/// An engine with issue `a` running and, when `token` is set, a status token.
pub(crate) fn live(root: &Path, token: Option<&str>, deadline: Duration) -> Live {
  let epic = root.join("epic");
  crate::disk::write_value(
    &epic.join("issues").join("a.json"),
    &json!({"title": "A", "url": "https://github.com/o/r/issues/1", "stage": "running"}),
  )
  .expect("issue");
  std::fs::create_dir_all(epic.join("status")).expect("status");
  let mut env = Env::from_pairs([("HOME", root.display().to_string())]);
  if let Some(token) = token {
    env = env.with("TOOLU_EPIC_STATUS_TOKEN", token);
  }
  let secrets = secrets::load(&Roots::new(env, None)).expect("secrets");
  let paths = Paths::at(root.join("resources"));
  let addr = std::net::TcpListener::bind("127.0.0.1:0")
    .and_then(|port| port.local_addr())
    .expect("port");
  let (background, scripts) = (paths.clone(), scripts(root));
  let engine = thread::spawn(move || {
    let http = Config {
      deadline,
      ..Config::new(addr, secrets, Vec::new())
    };
    let options = ServeOptions {
      http: Some(http),
      tick: &(),
      attention: crate::notify::AttentionOptions::default(),
    };
    serve_with_http(background, Some(scripts), Fault::None, PROTOCOL, options)
  });
  let report = json!({"op": "report", "key": "a", "epic": "one", "phase": "needs-human",
    "note": "which one?", "state_dir": epic.display().to_string()});
  let reply = exchange_retry(&paths, PROTOCOL, &report).expect("report");
  assert_eq!(reply["ok"], true);
  Live {
    paths,
    root: root.to_owned(),
    addr,
    engine,
  }
}

/// The headers an allowed dashboard request sends.
pub(crate) fn allowed(addr: SocketAddr) -> String {
  format!(
    "Authorization: Bearer {TOKEN}\r\nOrigin: http://{addr}\r\nContent-Type: application/json\r\n"
  )
}

/// One raw request; returns the status code and the whole response text.
pub(crate) fn raw(addr: SocketAddr, text: &str) -> (u16, String) {
  let mut stream = TcpStream::connect(addr).expect("connect");
  stream.write_all(text.as_bytes()).expect("request");
  let mut bytes = Vec::new();
  stream.read_to_end(&mut bytes).expect("response");
  let response = String::from_utf8(bytes).expect("utf-8");
  let status = response
    .get(9..12)
    .and_then(|code| code.parse().ok())
    .unwrap_or(0);
  (status, response)
}

/// `POST /api/v1/actions/{op}` with `headers` and a JSON `body`.
pub(crate) fn post(addr: SocketAddr, op: &str, headers: &str, body: &str) -> (u16, String) {
  raw(
    addr,
    &format!(
      "POST /api/v1/actions/{op} HTTP/1.1\r\nHost: {addr}\r\n{headers}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
      body.len()
    ),
  )
}

/// Every journal line so far.
pub(crate) fn journal(paths: &Paths) -> String {
  let mut files: Vec<PathBuf> = std::fs::read_dir(paths.journal_dir())
    .map(|dir| {
      dir
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .collect()
    })
    .unwrap_or_default();
  files.sort();
  files
    .iter()
    .map(|file| std::fs::read_to_string(file).unwrap_or_default())
    .collect()
}

/// Whether any file under `root` holds the token.
pub(crate) fn token_on_disk(root: &Path) -> bool {
  std::fs::read_dir(root).is_ok_and(|dir| {
    dir.filter_map(Result::ok).any(|entry| {
      let path = entry.path();
      if path.is_dir() {
        token_on_disk(&path)
      } else {
        std::fs::read(&path).is_ok_and(|bytes| String::from_utf8_lossy(&bytes).contains(TOKEN))
      }
    })
  })
}

/// Headers with an optional bearer and origin, then a content type.
fn headers(bearer: Option<&str>, origin: Option<&str>, content_type: &str) -> String {
  let bearer = bearer.map(|token| format!("Authorization: Bearer {token}\r\n"));
  let origin = origin.map(|origin| format!("Origin: {origin}\r\n"));
  format!(
    "{}{}Content-Type: {content_type}\r\n",
    bearer.unwrap_or_default(),
    origin.unwrap_or_default()
  )
}

/// Each gate refusal of a valid pause: its headers and status.
fn gate_refusals(addr: SocketAddr) -> [(String, u16); 5] {
  let same = format!("http://{addr}");
  let json = "application/json";
  [
    (headers(None, Some(&same), json), 401),
    (headers(Some("wrong"), Some(&same), json), 401),
    (headers(Some(TOKEN), Some("http://evil.example"), json), 403),
    (headers(Some(TOKEN), None, json), 403),
    (headers(Some(TOKEN), Some(&same), "text/plain"), 415),
  ]
}

/// Each body refusal with allowed headers: op, body and status.
const BODY_REFUSALS: [(&str, &str, u16); 7] = [
  ("nope", "{}", 404),
  ("pause", "[]", 400),
  ("pause", r#"{"epic": "bad key"}"#, 400),
  ("release", "{}", 400),
  ("ack", r#"{"key": "a", "kind": "budget"}"#, 400),
  ("answer", r#"{"key": "a", "text": ""}"#, 400),
  ("answer", r#"{"key": "a", "text": "ring\u0007"}"#, 400),
];

#[test]
fn every_refusal_answers_its_status_and_never_reaches_the_engine() {
  let temp = tempfile::tempdir().expect("temp");
  let live = live(temp.path(), Some(TOKEN), crate::http_action::DEADLINE);
  let addr = live.addr;
  let before = journal(&live.paths);
  let mut seen = String::new();
  for (headers, status) in gate_refusals(addr) {
    let (code, response) = post(addr, "pause", &headers, "{}");
    assert_eq!(code, status, "{headers} -> {response}");
    seen.push_str(&response);
  }
  let long = format!("{{\"key\": \"a\", \"text\": \"{}\"}}", "x".repeat(4_001));
  let bodies = BODY_REFUSALS
    .iter()
    .copied()
    .chain([("answer", long.as_str(), 400)]);
  for (op, body, status) in bodies {
    let (code, response) = post(addr, op, &allowed(addr), body);
    assert_eq!(code, status, "{op} {body} -> {response}");
    seen.push_str(&response);
  }
  let (code, response) = raw(
    addr,
    &format!("GET /api/v1/actions/pause HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n"),
  );
  assert_eq!(code, 405);
  assert!(response.contains("Allow: POST\r\n"), "{response}");
  let (code, _) = raw(
    addr,
    &format!(
      "POST /api/v1/actions/pause HTTP/1.1\r\nHost: {addr}\r\n{}\r\n",
      allowed(addr)
    ),
  );
  assert_eq!(code, 400, "a POST without Content-Length");
  // Headers only: the server refuses before reading, so no unread body resets it.
  let (code, _) = raw(
    addr,
    &format!(
      "POST /api/v1/actions/pause HTTP/1.1\r\nHost: {addr}\r\n{}Content-Length: 16385\r\n\r\n",
      allowed(addr)
    ),
  );
  assert_eq!(code, 413, "a body over 16 KiB");
  assert!(seen.contains("\"error\":\"foreign_origin\""));
  assert!(seen.contains("\"error\":\"unknown_op\""));
  assert!(!seen.contains("Access-Control-Allow"));
  assert_eq!(journal(&live.paths), before);
  assert!(!seen.contains(TOKEN));
  assert!(!token_on_disk(&live.root));
  live.stop();
}

#[test]
fn an_engine_without_a_token_refuses_control() {
  let temp = tempfile::tempdir().expect("temp");
  let live = live(temp.path(), None, crate::http_action::DEADLINE);
  let before = journal(&live.paths);
  let (code, response) = post(live.addr, "pause", &allowed(live.addr), "{}");
  assert_eq!(code, 403, "{response}");
  assert!(response.contains("\"error\":\"control_disabled\""));
  assert_eq!(journal(&live.paths), before);
  let http = live.paths.http();
  let bound = crate::disk::read_value(&http).expect("http.json");
  assert_eq!(bound, json!({"addr": live.addr.to_string()}));
  live.stop();
  assert!(!http.exists(), "a stopped engine removes http.json");
}
