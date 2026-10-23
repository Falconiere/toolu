use std::io::{ErrorKind, Write};
use std::os::unix::net::UnixListener;
use std::path::Path;
use std::time::{Duration, Instant, SystemTime};

use serde_json::{Value, json};
use toolu_http_test_support::{Fixture, Reply};
use toolu_runtime::env::Env;

use super::{Notifier, Settings, announce};
use crate::client::{read_json, write_json};
use crate::journal::{self, Record};
use crate::model::World;
use crate::notify_notice::{Notice, build};
use crate::paths::Paths;
use crate::server::{Engine, Fault};

const CANARY: &str = "canary-status-token-450";

fn env(home: &Path, url: Option<&str>) -> Env {
  let mut env = Env::from_pairs([
    ("HOME", home.display().to_string()),
    ("TOOLU_EPIC_STATUS_TOKEN", CANARY.to_owned()),
  ]);
  if let Some(url) = url {
    env = env.with("TOOLU_EPIC_NOTIFY_URL", url);
  }
  env
}

fn settings(env: Env, fixture: &Fixture) -> Settings {
  let env = env.with("HTTPS_PROXY", &fixture.proxy_url());
  Settings {
    backoff: Duration::from_millis(20),
    timeout: Duration::from_secs(2),
    test_root_ca_der: Some(fixture.root_ca_der().to_vec()),
    ..Settings::new(env, false, true)
  }
}

fn notice(note: &str) -> Notice {
  let mut world = World::new(0);
  let token = crate::attention::raise(&mut world, "needs-human", "a", note);
  build(&world, &token, false, Some("http://127.0.0.1:7717/")).expect("notice")
}

fn outcome(paths: &Paths, names: &[&str]) -> Record {
  let start = Instant::now();
  while start.elapsed() < Duration::from_secs(20) {
    let rows = journal::tail(&paths.journal_dir(), SystemTime::now()).expect("tail");
    if let Some(row) = rows
      .into_iter()
      .find(|row| names.contains(&row.name.as_str()))
    {
      return row;
    }
    std::thread::sleep(Duration::from_millis(20));
  }
  panic!("no {names:?} record");
}

/// Send one notice to a notifier posting to the fixture's `/hook`, until it is journaled.
fn post_once(root: &Path, fixture: &Fixture) -> Paths {
  let paths = Paths::at(root);
  let url = fixture.url("/hook");
  let notifier = Notifier::spawn(settings(env(root, Some(&url)), fixture), paths.clone());
  notifier.tx.try_send(notice("which?")).expect("send");
  let _row = outcome(&paths, &["notify", "notify-failed"]);
  paths
}

#[test]
fn notify_retries_server_errors_then_succeeds_with_a_redacted_body() {
  let tmp = tempfile::tempdir().expect("temp");
  let fixture = Fixture::start().expect("fixture");
  fixture
    .sequence(
      "/hook",
      vec![
        Reply::new(500, "no"),
        Reply::new(500, "no"),
        Reply::new(200, "ok"),
      ],
    )
    .expect("sequence");
  let url = fixture.url("/hook");
  let paths = Paths::at(tmp.path());
  let notifier = Notifier::spawn(
    settings(env(tmp.path(), Some(&url)), &fixture),
    paths.clone(),
  );
  notifier
    .tx
    .try_send(notice(&format!("token {CANARY} leaked")))
    .expect("send");
  let row = outcome(&paths, &["notify", "notify-failed"]);
  assert_eq!(row.name, "notify");
  assert_eq!(row.note, "channel=url attempts=3 status=200");
  let requests = fixture.requests().expect("requests");
  assert_eq!(requests.len(), 3);
  let body = String::from_utf8(requests[2].body.clone()).expect("utf8");
  assert!(!body.contains(CANARY), "{body}");
  let payload: Value = serde_json::from_str(&body).expect("json");
  assert_eq!(payload["kind"], "needs-human");
  assert_eq!(payload["dashboard"], "http://127.0.0.1:7717/");
  assert_eq!(requests[2].headers["content-type"], "application/json");
}

#[test]
fn notify_retries_dropped_connections_five_times_then_fails() {
  let tmp = tempfile::tempdir().expect("temp");
  let fixture = Fixture::start().expect("fixture");
  fixture.route("/hook", Reply::dropped()).expect("route");
  let paths = post_once(tmp.path(), &fixture);
  let row = outcome(&paths, &["notify", "notify-failed"]);
  assert_eq!(row.name, "notify-failed");
  assert_eq!(row.note, "channel=url attempts=5 status=transport");
}

#[test]
fn notify_fails_at_once_on_a_client_error_and_refuses_a_bad_url() {
  let tmp = tempfile::tempdir().expect("temp");
  let fixture = Fixture::start().expect("fixture");
  fixture
    .route("/hook", Reply::new(404, "gone"))
    .expect("route");
  let paths = post_once(tmp.path(), &fixture);
  let row = outcome(&paths, &["notify", "notify-failed"]);
  assert_eq!(row.note, "channel=url attempts=1 status=404");
  let other = tempfile::tempdir().expect("temp");
  let paths = Paths::at(other.path());
  let bad = env(other.path(), Some("ftp://hooks.example/x"));
  let notifier = Notifier::spawn(settings(bad, &fixture), paths.clone());
  notifier.tx.try_send(notice("which?")).expect("send");
  let row = outcome(&paths, &["notify", "notify-failed"]);
  assert_eq!(row.note, "channel=url attempts=0 status=refused");
}

#[test]
fn notify_without_a_url_posts_nothing_and_a_bad_secrets_file_is_journaled() {
  let tmp = tempfile::tempdir().expect("temp");
  let fixture = Fixture::start().expect("fixture");
  let paths = Paths::at(tmp.path());
  let (tx, rx) = std::sync::mpsc::sync_channel(1);
  tx.send(notice("which?")).expect("send");
  drop(tx);
  super::run(&rx, &settings(env(tmp.path(), None), &fixture), &paths);
  assert_eq!(fixture.requests().expect("requests").len(), 0);
  assert_eq!(
    journal::tail(&paths.journal_dir(), SystemTime::now()).expect("tail"),
    []
  );
  let broken = env(tmp.path(), None).with("TOOLU_EPIC_PEER_TOKENS", "not json");
  let notifier = Notifier::spawn(settings(broken, &fixture), paths.clone());
  notifier.tx.try_send(notice("which?")).expect("send");
  let row = outcome(&paths, &["notify", "notify-failed"]);
  assert_eq!(row.note, "channel=url attempts=0 status=secrets");
  assert_eq!(fixture.requests().expect("requests").len(), 0);
}

#[test]
fn herdr_notification_shows_one_titled_request() {
  let tmp = tempfile::tempdir().expect("temp");
  let sock = tmp.path().join("herdr.sock");
  let listener = UnixListener::bind(&sock).expect("bind");
  let seen = tmp.path().join("seen.jsonl");
  let record = seen.clone();
  let peer = std::thread::spawn(move || herdr(&listener, &record));
  let paths = Paths::at(tmp.path());
  let herdr_env = env(tmp.path(), None).with("HERDR_SOCKET_PATH", &sock.display().to_string());
  let notifier = Notifier::spawn(Settings::new(herdr_env, true, false), paths.clone());
  notifier.tx.try_send(notice("which one?")).expect("send");
  let row = outcome(&paths, &["notify", "notify-failed"]);
  assert_eq!(row.note, "channel=herdr attempts=1 status=ok");
  peer.join().expect("peer");
  let requests: Vec<Value> = std::fs::read_to_string(&seen)
    .expect("seen")
    .lines()
    .map(|line| serde_json::from_str(line).expect("json"))
    .collect();
  assert_eq!(requests.len(), 1);
  assert_eq!(requests[0]["method"], "notification.show");
  assert_eq!(
    requests[0]["params"],
    json!({"title": "toolu epic: a needs-human", "body": "which one?", "sound": "request"})
  );
}

#[test]
fn herdr_notification_is_not_sent_when_herdr_is_off() {
  let tmp = tempfile::tempdir().expect("temp");
  let sock = tmp.path().join("herdr.sock");
  let listener = UnixListener::bind(&sock).expect("bind");
  listener.set_nonblocking(true).expect("nonblocking");
  let paths = Paths::at(tmp.path());
  let herdr_env = env(tmp.path(), None).with("HERDR_SOCKET_PATH", &sock.display().to_string());
  let notifier = Notifier::spawn(Settings::new(herdr_env, false, false), paths);
  notifier.tx.try_send(notice("which one?")).expect("send");
  drop(notifier);
  std::thread::sleep(Duration::from_millis(200));
  let accepted = listener.accept().map(|_| ());
  assert_eq!(
    accepted.map_err(|err| err.kind()),
    Err(ErrorKind::WouldBlock)
  );
}

#[test]
fn a_full_queue_drops_the_notice_and_journals_it_without_waiting() {
  let tmp = tempfile::tempdir().expect("temp");
  let fixture = Fixture::start().expect("fixture");
  fixture
    .route(
      "/hook",
      Reply::new(200, "ok").delayed(Duration::from_secs(1)),
    )
    .expect("route");
  let url = fixture.url("/hook");
  let mut engine = Engine::open(Paths::at(tmp.path()), None, Fault::None).expect("open");
  let settings = settings(env(tmp.path(), Some(&url)), &fixture);
  engine.notifier = Some(Notifier::spawn(settings, engine.paths.clone()));
  let token = crate::attention::raise(&mut engine.world, "failed", "a", "boom");
  engine.world.outbox.clear();
  let start = Instant::now();
  for _ in 0..70 {
    announce(&mut engine, &token, true);
  }
  assert!(start.elapsed() < Duration::from_millis(500));
  let dropped: Vec<String> = engine
    .world
    .outbox
    .iter()
    .filter_map(|step| match step {
      crate::model::Step::Journal(row) if row.name == "notify-dropped" => Some(row.note.clone()),
      crate::model::Step::Journal(_)
      | crate::model::Step::Call { .. }
      | crate::model::Step::Status { .. }
      | crate::model::Step::Issue { .. }
      | crate::model::Step::Prompt { .. }
      | crate::model::Step::Notify { .. } => None,
    })
    .collect();
  assert!(dropped.len() >= 5, "{}", dropped.len());
  assert_eq!(dropped[0], "channel=queue attempts=0 status=full");
  announce(&mut engine, "a-gone", false);
  engine.notifier = None;
  announce(&mut engine, &token, false);
}

/// Answer one request per connection, as herdr 0.9.3 does, until a non-ping
/// request arrives.
fn herdr(listener: &UnixListener, seen: &Path) {
  for _ in 0..2 {
    let Ok((mut stream, _)) = listener.accept() else {
      return;
    };
    let Ok(request) = read_json(&mut stream) else {
      continue;
    };
    let method = request.get("method").and_then(Value::as_str).unwrap_or("");
    let id = request.get("id").cloned().unwrap_or(Value::Null);
    let result = if method == "ping" {
      json!({"type": "pong", "protocol": 22})
    } else {
      json!({})
    };
    if write_json(&mut stream, &json!({"id": id, "result": result})).is_err() || method == "ping" {
      continue;
    }
    let mut file = std::fs::OpenOptions::new()
      .create(true)
      .append(true)
      .open(seen)
      .expect("seen");
    writeln!(file, "{request}").expect("write");
    return;
  }
}
