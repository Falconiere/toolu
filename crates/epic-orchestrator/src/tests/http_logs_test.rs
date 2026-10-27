use std::io::{Read as _, Write as _};
use std::net::{SocketAddr, TcpStream};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Condvar, Mutex};

use serde_json::Value;
use toolu_engine::resources::binding::ResourceBinding;
use toolu_engine::resources::jobs::run_managed_job;
use toolu_runtime::process::Spec;

use super::{Chunk, Log, Source, bounded, detail};
use crate::fleet_rig::secrets_of;
use crate::http::{Config, Running, start};
use crate::model::{Issue, World};
use crate::paths::Paths;

const TOKEN: &str = "SECRET-LOG-CANARY";

fn get(addr: SocketAddr, path: &str, token: Option<&str>) -> (u16, Value) {
  let mut stream = TcpStream::connect(addr).unwrap();
  let auth = token.map_or_else(String::new, |token| {
    format!("Authorization: Bearer {token}\r\n")
  });
  write!(
    stream,
    "GET {path} HTTP/1.1\r\nHost: {addr}\r\n{auth}Connection: close\r\n\r\n"
  )
  .unwrap();
  let mut response = String::new();
  stream.read_to_string(&mut response).unwrap();
  let (head, body) = response.split_once("\r\n\r\n").unwrap();
  (
    head.get(9..12).unwrap().parse().unwrap(),
    serde_json::from_str(body).unwrap(),
  )
}

fn job_listener() -> (tempfile::TempDir, Running, Arc<AtomicBool>, String) {
  let dir = tempfile::tempdir().unwrap();
  let binding = ResourceBinding {
    root: dir.path().to_path_buf(),
    key: "worker".to_owned(),
    state_dir: dir.path().join("epic").display().to_string(),
    worktree: dir.path().to_path_buf(),
  };
  let argv = vec![
    "sh".to_owned(),
    "-c".to_owned(),
    format!("printf '{TOKEN}'"),
  ];
  let output = run_managed_job(&argv, &binding, &Spec::new(Vec::<String>::new())).unwrap();
  assert_eq!(output.stdout, TOKEN);
  let mut other = binding.clone();
  other.key = "other".to_owned();
  run_managed_job(
    &["true".to_owned()],
    &other,
    &Spec::new(Vec::<String>::new()),
  )
  .unwrap();
  let foreign_id = toolu_engine::resources::job_logs::list(dir.path(), "other").unwrap()[0]
    .id
    .clone();
  let secrets = secrets_of(dir.path(), &[("TOOLU_EPIC_STATUS_TOKEN", TOKEN)]);
  let config = Config::new("127.0.0.1:0".parse().unwrap(), secrets, Vec::new());
  let mut world = World::new(0);
  let issue = Issue::blank("worker", "epic", &binding.state_dir, 0);
  world.issues.insert(issue.key.clone(), issue);
  let stop = Arc::new(AtomicBool::new(false));
  let listener = start(
    config,
    Paths::at(dir.path()),
    Arc::new(Mutex::new(world)),
    Arc::new(Condvar::new()),
    Arc::clone(&stop),
  )
  .unwrap();
  (dir, listener, stop, foreign_id)
}

fn assert_invalid_ids(addr: SocketAddr, path: &str, foreign_id: &str) {
  for bad in ["../outside", "/etc/passwd", "unknown"] {
    assert_eq!(get(addr, &format!("{path}/{bad}"), Some(TOKEN)).0, 404);
  }
  assert_eq!(get(addr, "/api/v1/issues/other/logs", Some(TOKEN)).0, 404);
  assert_eq!(
    get(addr, &format!("{path}/{foreign_id}"), Some(TOKEN)).0,
    404
  );
}

#[test]
fn a_real_managed_job_is_listed_and_redacted_only_with_a_token() {
  let (_dir, listener, stop, foreign_id) = job_listener();
  let path = "/api/v1/issues/worker/logs";
  assert_eq!(get(listener.addr, path, None).0, 401);
  assert_eq!(get(listener.addr, path, Some("wrong")).0, 401);
  let (status, listed) = get(listener.addr, path, Some(TOKEN));
  assert_eq!(status, 200, "{listed}");
  assert_eq!(listed["items"].as_array().unwrap().len(), 1);
  assert_eq!(listed["items"][0]["source"], "job");
  assert_eq!(listed["items"][0]["state"], "done");
  assert!(!listed.to_string().contains(TOKEN));
  let id = listed["items"][0]["id"].as_str().unwrap();
  let detail = format!("{path}/{id}?offset=0");
  assert_eq!(get(listener.addr, &detail, None).0, 401);
  assert_eq!(get(listener.addr, &detail, Some("wrong")).0, 401);
  let (status, body) = get(listener.addr, &detail, Some(TOKEN));
  assert_eq!(status, 200);
  assert_eq!(body["text"], "<redacted>");
  assert_invalid_ids(listener.addr, path, &foreign_id);
  listener.stop(&stop).unwrap();
}

#[test]
fn fleet_peer_worker_logs_are_never_proxied() {
  let dir = tempfile::tempdir().unwrap();
  let peer = crate::peer_fixture::peer("box", "127.0.0.1:9".parse().unwrap());
  let (listener, stop) = crate::fleet_rig::listener(
    dir.path(),
    crate::fleet_rig::config(
      0,
      vec![peer],
      toolu_runtime::config::secrets::Secrets::none(),
    ),
  );
  assert_eq!(
    get(
      listener.addr,
      "/api/v1/fleet/peers/box/issues/worker/logs",
      None
    )
    .0,
    404
  );
  listener.stop(&stop).unwrap();
}

#[test]
fn unreadable_selected_log_is_available_false_and_expanded_json_is_bounded() {
  let missing = Log {
    id: "missing".to_owned(),
    source: Source::Claude("/no/such/file".into()),
    label: "missing".to_owned(),
    state: "done",
    size: 0,
    started_at: String::new(),
    updated_at: String::new(),
  };
  let secrets = toolu_runtime::config::secrets::Secrets::none();
  let reply = detail(&[missing], "missing", "", &secrets);
  let body: Value = serde_json::from_slice(&reply.body).unwrap();
  assert_eq!(reply.status, 200);
  assert_eq!(body["available"], false);
  let huge = Chunk {
    bytes: vec![0; 60 * 1024],
    offset: 0,
    next_offset: 60 * 1024,
    size: 60 * 1024,
    truncated: false,
  };
  let reply = bounded(&huge, 0, None, "running", &secrets);
  assert!(reply.body.len() <= 64 * 1024);
  let body: Value = serde_json::from_slice(&reply.body).unwrap();
  assert!(body["next_offset"].as_u64().unwrap() < 60 * 1024);
}

#[test]
fn a_secret_split_across_a_page_edge_is_redacted_on_both_sides() {
  let dir = tempfile::tempdir().unwrap();
  let secrets = secrets_of(dir.path(), &[("TOOLU_EPIC_STATUS_TOKEN", TOKEN)]);
  let prefix = vec![b'a'; 60 * 1024 - 2];
  let mut bytes = prefix;
  bytes.extend_from_slice(TOKEN.as_bytes());
  bytes.extend_from_slice(b"tail");
  let raw = bytes.clone();
  let size = bytes.len() as u64;
  let chunk = Chunk {
    bytes,
    offset: 0,
    next_offset: size,
    size,
    truncated: false,
  };
  let first = bounded(&chunk, 0, Some(0), "done", &secrets);
  let first: Value = serde_json::from_slice(&first.body).unwrap();
  assert!(first["text"].as_str().unwrap().ends_with("<redacted>"));
  let chunk = Chunk {
    bytes: format!("left{TOKEN}right").into_bytes(),
    offset: 0,
    next_offset: 4 + TOKEN.len() as u64 + 5,
    size: 4 + TOKEN.len() as u64 + 5,
    truncated: false,
  };
  let second = bounded(&chunk, 6, Some(6), "done", &secrets);
  let second: Value = serde_json::from_slice(&second.body).unwrap();
  assert_eq!(second["text"], "<redacted>right");
  let writer =
    toolu_engine::resources::job_logs::start(dir.path(), "cross", "worker", &["cross".to_owned()])
      .unwrap();
  writer.append(&raw).unwrap();
  let issue = Issue::blank("worker", "epic", "/tmp/epic", 0);
  let logs = super::discover(dir.path(), &issue, &secrets).unwrap();
  let response = detail(&logs, "job-cross", "offset=0", &secrets);
  let body: Value = serde_json::from_slice(&response.body).unwrap();
  assert_eq!(response.status, 200);
  assert!(body["text"].as_str().unwrap().ends_with("<redacted>"));
}
