//! Fleet test rig: real in-process engines with a scripted epic, bare status
//! listeners, peer secrets and raw HTTP reads.

use std::io::{Read as _, Write as _};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use serde_json::{Value, json};
use toolu_runtime::config::epic::Peer;
use toolu_runtime::config::secrets::{self, Secrets};
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;

use crate::PROTOCOL;
use crate::client::exchange_retry;
use crate::disk::write_value;
use crate::http::{Config, Running, start};
use crate::model::World;
use crate::paths::Paths;
use crate::server::Fault;
use crate::socket::{ServeOptions, serve_with_http};

/// One running engine with a registered epic and a `running` issue.
pub(crate) struct Engine {
  pub paths: Paths,
  pub addr: SocketAddr,
  pub root: PathBuf,
  thread: JoinHandle<Result<(), String>>,
}

impl Engine {
  /// Register `epic` with `issue` under `root`, serve HTTP on `bind`, and report
  /// `issue` in execution so its stage is `running`.
  pub(crate) fn start(root: &Path, epic: &str, issue: &str, http: Config) -> Engine {
    let graph =
      json!({"issues": [{"key": issue, "title": "Fleet", "wave": 1, "open_blockers": []}]});
    Self::start_with(root, (epic, issue), &graph, http)
  }

  /// As [`Engine::start`], with a complete graph whose epic is titled `title`.
  /// The epic is paused, so the engine launches nothing before the report.
  pub(crate) fn start_titled(
    root: &Path,
    (epic, title): (&str, &str),
    issue: &str,
    http: Config,
  ) -> Engine {
    let graph = json!({
      "epic": {"ref": "o/r#9", "title": title, "state": "open", "url": "https://github.com/o/r/issues/9"},
      "state_dir": root.join(epic),
      "issues": [{"ref": "o/r#10", "title": "Fleet", "state": "open", "url": "https://github.com/o/r/issues/10",
        "repo": "o/r", "number": 10, "key": issue, "wave": 1, "open_blockers": []}]
    });
    let paths = Paths::at(root.join("resources"));
    write_value(&paths.pause(), &json!({"epics": [epic]})).expect("pause");
    Self::start_with(root, (epic, issue), &graph, http)
  }

  fn start_with(root: &Path, (epic, issue): (&str, &str), graph: &Value, http: Config) -> Engine {
    let paths = Paths::at(root.join("resources"));
    let state = root.join(epic);
    std::fs::create_dir_all(state.join("status")).expect("status dir");
    write_value(
      &paths.registry(),
      &json!({"epics": [{"key": epic, "state_dir": state}]}),
    )
    .expect("registry");
    write_value(&state.join("graph.json"), graph).expect("graph");
    let addr = SocketAddr::from(([127, 0, 0, 1], http.addr.port()));
    let token = http.secrets.status_token().map(str::to_owned);
    let background = paths.clone();
    let thread = thread::spawn(move || {
      let options = ServeOptions {
        http: Some(http),
        tick: &(),
        attention: crate::notify::AttentionOptions::default(),
      };
      serve_with_http(background, None, Fault::None, PROTOCOL, options)
    });
    wait_for(addr);
    let report = json!({"op": "report", "key": issue, "epic": epic,
      "state_dir": state.display().to_string(), "phase": "execution", "note": "fleet"});
    let reply = exchange_retry(&paths, PROTOCOL, &report).expect("report");
    assert_eq!(reply["ok"], true, "{reply}");
    wait_for_report(addr, issue, token.as_deref());
    Engine {
      paths,
      addr,
      root: root.to_path_buf(),
      thread,
    }
  }

  /// This engine as the config peer `name`.
  pub(crate) fn peer(&self, name: &str) -> Peer {
    crate::peer_fixture::peer(name, self.addr)
  }

  /// Stop the engine and wait for its listener to close.
  pub(crate) fn stop(self) {
    exchange_retry(&self.paths, PROTOCOL, &json!({"op": "stop"})).expect("stop");
    self
      .thread
      .join()
      .expect("engine thread")
      .expect("engine result");
  }
}

fn wait_for_report(addr: SocketAddr, issue: &str, token: Option<&str>) {
  for _ in 0..200 {
    let (code, body) = get_with_token(addr, "/api/v1/overview", token);
    assert_eq!(code, 200);
    let status: Value = serde_json::from_str(&body).expect("overview");
    let published = status["issues"].as_array().is_some_and(|issues| {
      issues
        .iter()
        .any(|row| row["key"] == issue && row["stage"] == "running")
    });
    if published {
      return;
    }
    thread::sleep(Duration::from_millis(10));
  }
  panic!("worker report {issue} was not published");
}

/// A free loopback port, released for the caller to bind.
pub(crate) fn free_port() -> u16 {
  let reserved = TcpListener::bind("127.0.0.1:0").expect("port");
  reserved.local_addr().expect("address").port()
}

/// A listener config on loopback `port` with `peers` and `secrets`.
pub(crate) fn config(port: u16, peers: Vec<Peer>, secrets: Secrets) -> Config {
  Config::new(SocketAddr::from(([127, 0, 0, 1], port)), secrets, peers)
}

/// Secrets resolved from `HOME` and `pairs`, as `secrets::load` reads them.
pub(crate) fn secrets_of(home: &Path, pairs: &[(&str, &str)]) -> Secrets {
  let mut env = Env::from_pairs([("HOME", home.display().to_string())]);
  for (key, value) in pairs {
    env = env.with(key, value);
  }
  secrets::load(&Roots::new(env, None)).expect("secrets")
}

/// A bare status listener over an empty world, and its stop flag.
pub(crate) fn listener(root: &Path, config: Config) -> (Running, Arc<AtomicBool>) {
  let stop = Arc::new(AtomicBool::new(false));
  let running = start(
    config,
    Paths::at(root),
    Arc::new(Mutex::new(World::new(0))),
    Arc::new(Condvar::new()),
    Arc::clone(&stop),
  )
  .expect("listener");
  (running, stop)
}

fn wait_for(addr: SocketAddr) {
  for _ in 0..200 {
    if TcpStream::connect(addr).is_ok() {
      return;
    }
    thread::sleep(Duration::from_millis(10));
  }
  panic!("listener {addr} did not start");
}

/// `GET path` with `Host: addr`; the status code and the raw body.
pub(crate) fn get(addr: SocketAddr, path: &str) -> (u16, String) {
  get_with_token(addr, path, None)
}

fn get_with_token(addr: SocketAddr, path: &str, token: Option<&str>) -> (u16, String) {
  let mut stream = TcpStream::connect(addr).expect("connect");
  let auth = token
    .map(|token| format!("Authorization: Bearer {token}\r\n"))
    .unwrap_or_default();
  let request = format!("GET {path} HTTP/1.1\r\nHost: {addr}\r\n{auth}Connection: close\r\n\r\n");
  stream.write_all(request.as_bytes()).expect("request");
  let mut bytes = Vec::new();
  stream.read_to_end(&mut bytes).expect("response");
  let text = String::from_utf8(bytes).expect("utf-8");
  let status = text
    .get(9..12)
    .and_then(|code| code.parse().ok())
    .unwrap_or(0);
  let body = text.split_once("\r\n\r\n").map_or("", |(_, body)| body);
  (status, body.to_owned())
}

/// `GET path` decoded as JSON, asserting `200`.
pub(crate) fn get_json(addr: SocketAddr, path: &str) -> Value {
  let (status, body) = get(addr, path);
  assert_eq!(status, 200, "{path}: {body}");
  serde_json::from_str(&body).expect("json body")
}

/// Every file under `root`, read lossily, joined.
pub(crate) fn files_text(root: &Path) -> String {
  let mut text = String::new();
  let mut dirs = vec![root.to_path_buf()];
  while let Some(dir) = dirs.pop() {
    for entry in std::fs::read_dir(&dir).expect("read dir").flatten() {
      let path = entry.path();
      if path.is_dir() {
        dirs.push(path);
      } else if let Ok(bytes) = std::fs::read(&path) {
        text.push_str(&String::from_utf8_lossy(&bytes));
      }
    }
  }
  text
}

/// The machine named `name` in a fleet document.
pub(crate) fn machine<'a>(fleet: &'a Value, name: &str) -> &'a Value {
  fleet["machines"]
    .as_array()
    .and_then(|machines| machines.iter().find(|machine| machine["name"] == name))
    .expect("machine")
}
