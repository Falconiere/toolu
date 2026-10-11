use std::io::{Read as _, Write as _};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::mpsc::channel;
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use toolu_http_test_support::{Fixture, ObservedRequest, Reply};
use toolu_runtime::config::secrets;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;

use super::{reply_ack, reply_answer};
use crate::PROTOCOL;
use crate::client::exchange_retry;
use crate::journal::{self, Record};
use crate::model::{Ack, Report};
use crate::paths::Paths;
use crate::server::{Engine, Fault};
use crate::socket::{AttentionOptions, ServeOptions, serve_with_http};

const CANARY: &str = "canary-status-token-450";

fn prompt_scripts(root: &Path, outcome: &str) -> PathBuf {
  let scripts = root.join("scripts");
  std::fs::create_dir_all(&scripts).expect("scripts");
  std::fs::create_dir_all(root.join("epic/status")).expect("status");
  let script = format!(
    "cat >> '{}'\necho >> '{}'\nprintf '{{\"outcome\":\"{outcome}\"}}'\n",
    root.join("prompts").display(),
    root.join("prompts").display(),
  );
  std::fs::write(scripts.join("prompt"), script).expect("script");
  std::fs::write(
    scripts.join("launch"),
    "printf '{\"outcome\":\"applied\"}'\n",
  )
  .expect("launch");
  scripts
}

fn engine(root: &Path, outcome: &str) -> Engine {
  let scripts = prompt_scripts(root, outcome);
  Engine::open(Paths::at(root), Some(scripts), Fault::None).expect("open")
}

fn report(engine: &mut Engine, phase: &str) {
  let state_dir = engine.paths.root.join("epic").display().to_string();
  engine
    .report(&Report {
      key: "a".to_owned(),
      epic: "one".to_owned(),
      state_dir,
      phase: phase.to_owned(),
      pr: None,
      note: "which one?".to_owned(),
    })
    .expect("report");
}

fn answer(engine: &mut Engine, request: &Value) -> Value {
  let (tx, rx) = channel();
  assert!(!reply_answer(engine, request, &tx));
  rx.recv().expect("reply")
}

fn rows(engine: &Engine) -> Vec<Record> {
  journal::tail(&engine.paths.journal_dir(), engine.now).expect("tail")
}

fn prompts(root: &Path) -> Vec<Value> {
  std::fs::read_to_string(root.join("prompts"))
    .unwrap_or_default()
    .lines()
    .map(|line| serde_json::from_str(line).expect("prompt json"))
    .collect()
}

#[test]
fn answer_reaches_the_prompt_script_and_the_next_report_closes_it() {
  let tmp = tempfile::tempdir().expect("temp");
  let mut engine = engine(tmp.path(), "sent");
  report(&mut engine, "needs-human");
  let token = engine.world.attention[0].token.clone();
  let request = json!({"op": "answer", "key": "a", "text": "take B", "by": "user"});
  assert_eq!(answer(&mut engine, &request), json!({"ok": true}));
  assert_eq!(prompts(tmp.path()), [json!({"key": "a", "text": "take B"})]);
  assert_eq!(engine.world.attention[0].state, Ack::Answered);
  let journal = rows(&engine);
  let answered: Vec<(&str, &str, &str)> = journal
    .iter()
    .filter(|row| row.token == token && matches!(row.name.as_str(), "answer" | "answered"))
    .map(|row| (row.kind.as_str(), row.name.as_str(), row.note.as_str()))
    .collect();
  assert_eq!(
    answered,
    [
      ("prompt", "answer", "take B"),
      ("judgment", "answered", "by=user")
    ]
  );
  report(&mut engine, "execution");
  assert_eq!(engine.world.attention.len(), 0);
  let closed = rows(&engine)
    .into_iter()
    .find(|row| row.name == "closed")
    .expect("closed");
  assert_eq!(closed.token, token);
  assert!(
    closed.note.starts_with("kind=needs-human waited_ms="),
    "{}",
    closed.note
  );
}

#[test]
fn answer_without_a_live_item_calls_no_prompt() {
  let tmp = tempfile::tempdir().expect("temp");
  let mut engine = engine(tmp.path(), "sent");
  let request = json!({"op": "answer", "key": "a", "text": "take B"});
  assert_eq!(
    answer(&mut engine, &request),
    json!({"error": "no open attention for a"})
  );
  assert_eq!(prompts(tmp.path()), [] as [Value; 0]);
}

#[test]
fn answer_not_delivered_leaves_the_item_open() {
  let tmp = tempfile::tempdir().expect("temp");
  let mut engine = engine(tmp.path(), "failed");
  report(&mut engine, "needs-human");
  let request = json!({"op": "answer", "key": "a", "text": "take B"});
  assert_eq!(
    answer(&mut engine, &request),
    json!({"error": "worker prompt not delivered: prompt failed"})
  );
  assert_eq!(engine.world.attention[0].state, Ack::Open);
  assert!(!rows(&engine).iter().any(|row| row.name == "answered"));
}

#[test]
fn ack_journals_who_acknowledged() {
  let tmp = tempfile::tempdir().expect("temp");
  let mut engine = engine(tmp.path(), "sent");
  report(&mut engine, "failed");
  let (tx, rx) = channel();
  let request = json!({"op": "ack", "key": "a", "by": "cursor"});
  assert_eq!(reply_ack(&mut engine, &request, &tx), Ok(false));
  assert_eq!(rx.recv().expect("reply"), json!({"ok": true}));
  assert_eq!(engine.world.attention[0].state, Ack::Acknowledged);
  let acked = rows(&engine)
    .into_iter()
    .find(|row| row.name == "acknowledged")
    .expect("acknowledged");
  assert_eq!(acked.note, "by=cursor");
}

struct Live {
  paths: Paths,
  epic: PathBuf,
  addr: SocketAddr,
  engine: thread::JoinHandle<Result<(), String>>,
}

fn live(root: &Path, fixture: &Fixture) -> Live {
  let epic = root.join("epic");
  let scripts = prompt_scripts(root, "sent");
  crate::disk::write_value(
    &epic.join("issues").join("a.json"),
    &json!({"title": "A", "url": "https://github.com/o/r/issues/1"}),
  )
  .expect("issue");
  let env = Env::from_pairs([
    ("HOME", root.display().to_string()),
    ("TOOLU_EPIC_STATUS_TOKEN", CANARY.to_owned()),
    ("TOOLU_EPIC_NOTIFY_URL", fixture.url("/hook")),
    ("HTTPS_PROXY", fixture.proxy_url()),
  ]);
  let secrets = secrets::load(&Roots::new(env.clone(), None)).expect("secrets");
  let notify = crate::notify::Settings {
    test_root_ca_der: Some(fixture.root_ca_der().to_vec()),
    ..crate::notify::Settings::new(env, false, true)
  };
  let paths = Paths::at(root.join("resources"));
  let reserved = std::net::TcpListener::bind("127.0.0.1:0").expect("port");
  let addr = reserved.local_addr().expect("address");
  drop(reserved);
  let background = paths.clone();
  let engine = thread::spawn(move || {
    let http = Some(crate::http::Config { addr, secrets });
    let attention = AttentionOptions {
      remind_ms: crate::model::REMIND_MS,
      notify: Some(notify),
    };
    let options = ServeOptions {
      http,
      tick: &(),
      attention,
    };
    serve_with_http(background, Some(scripts), Fault::None, PROTOCOL, options)
  });
  Live {
    paths,
    epic,
    addr,
    engine,
  }
}

fn send(live: &Live, request: &Value) -> Value {
  exchange_retry(&live.paths, PROTOCOL, request).expect("exchange")
}

fn socket_report(live: &Live, phase: &str, note: &str) {
  let request = json!({"op": "report", "key": "a", "epic": "one", "phase": phase, "note": note,
    "state_dir": live.epic.display().to_string()});
  assert_eq!(send(live, &request)["ok"], true);
}

fn get(addr: SocketAddr, path: &str) -> Value {
  let mut stream = TcpStream::connect(addr).expect("connect");
  let request = format!("GET {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n");
  stream.write_all(request.as_bytes()).expect("request");
  let mut text = String::new();
  stream.read_to_string(&mut text).expect("response");
  let (_head, body) = text.split_once("\r\n\r\n").expect("body");
  serde_json::from_str(body).expect("json body")
}

fn posts(fixture: &Fixture, within: Duration) -> Vec<ObservedRequest> {
  let start = Instant::now();
  loop {
    let seen = fixture.requests().expect("requests");
    if !seen.is_empty() || start.elapsed() > within {
      return seen;
    }
    thread::sleep(Duration::from_millis(10));
  }
}

#[test]
fn attention_reaches_every_channel() {
  let tmp = tempfile::tempdir().expect("temp");
  let fixture = Fixture::start().expect("fixture");
  fixture
    .route("/hook", Reply::new(200, "ok"))
    .expect("route");
  let live = live(tmp.path(), &fixture);
  let start = Instant::now();
  socket_report(&live, "needs-human", &format!("which one? {CANARY}"));
  let wait = send(&live, &json!({"op": "wait", "max_seconds": 0}));
  assert_eq!(
    (&wait["kind"], &wait["state"]),
    (&json!("needs-human"), &json!("open"))
  );
  let listed = get(live.addr, "/api/v1/attention");
  assert_eq!(listed["items"][0]["key"], "a");
  assert!(listed["items"][0]["age_ms"].is_u64(), "{listed}");
  let seen = posts(&fixture, Duration::from_secs(2));
  assert!(start.elapsed() < Duration::from_secs(2));
  assert_post(&seen, live.addr);
  let answer = json!({"op": "answer", "key": "a", "text": "take B", "by": "user"});
  assert_eq!(send(&live, &answer), json!({"ok": true}));
  assert_eq!(prompts(tmp.path()), [json!({"key": "a", "text": "take B"})]);
  socket_report(&live, "execution", "working");
  assert_eq!(get(live.addr, "/api/v1/attention")["items"], json!([]));
  assert_eq!(send(&live, &json!({"op": "stop"}))["ok"], true);
  live.engine.join().expect("engine").expect("serve");
  assert_journal(&live.paths);
  assert_eq!(fixture.requests().expect("requests").len(), 1);
}

fn assert_post(seen: &[ObservedRequest], addr: SocketAddr) {
  assert_eq!(seen.len(), 1);
  let text = String::from_utf8(seen[0].body.clone()).expect("utf-8");
  assert!(!text.contains(CANARY), "{text}");
  let body: Value = serde_json::from_str(&text).expect("post json");
  assert_eq!(
    (&body["kind"], &body["key"]),
    (&json!("needs-human"), &json!("a"))
  );
  assert_eq!(body["issue"]["url"], "https://github.com/o/r/issues/1");
  assert_eq!(body["dashboard"], format!("http://{addr}/"));
  assert!(
    body["note"]
      .as_str()
      .is_some_and(|note| note.starts_with("which one? "))
  );
}

fn assert_journal(paths: &Paths) {
  let rows = journal::tail(&paths.journal_dir(), std::time::SystemTime::now()).expect("tail");
  let names: Vec<&str> = rows
    .iter()
    .filter(|row| row.kind == "judgment" || row.name == "notify")
    .map(|row| row.name.as_str())
    .collect();
  assert_eq!(names, ["needs-human", "notify", "answered", "closed"]);
}
