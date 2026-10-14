use std::net::SocketAddr;
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use toolu_runtime::config::secrets::Secrets;

use crate::fleet_rig::{
  Engine, config, files_text, free_port, get, get_json, listener, machine, secrets_of,
};
use crate::peer_fixture::{blackhole, peer, serve, serve_after};

fn overview(issue: &str, host: &str) -> String {
  json!({
    "schema": 1, "engine": "running", "paused": false, "journal_seq": 1,
    "issues": [{"key": issue, "epic": "repo-9", "stage": "running", "phase": "spec"}],
    "hosts": {"items": [{"host": host, "running": 1, "cap": 2}]},
    "counts": {"running": 1}, "attention_count": 0
  })
  .to_string()
}

fn keys(machine: &Value) -> Vec<&str> {
  machine["in_flight"]
    .as_array()
    .into_iter()
    .flatten()
    .filter_map(|issue| issue["key"].as_str())
    .collect()
}

#[test]
fn fleet_two_engines() {
  let temp = tempfile::tempdir().expect("temp root");
  let b_config = config(free_port(), Vec::new(), Secrets::none());
  let b = Engine::start(&temp.path().join("b"), "repo-500", "repo-501", b_config);
  let a_config = config(free_port(), vec![b.peer("server")], Secrets::none());
  let a = Engine::start(&temp.path().join("a"), "repo-402", "repo-449", a_config);
  let fleet = get_json(a.addr, "/api/v1/fleet");
  assert_eq!(fleet["schema"], 1);
  let local = machine(&fleet, "local");
  let server = machine(&fleet, "server");
  assert_eq!(
    (local["local"].clone(), local["state"].clone()),
    (json!(true), json!("ok"))
  );
  assert_eq!(
    (server["local"].clone(), server["state"].clone()),
    (json!(false), json!("ok"))
  );
  assert_eq!(keys(local), ["repo-449"]);
  assert_eq!(keys(server), ["repo-501"]);
  assert_eq!(server["in_flight"][0]["stage"], "running");
  assert_eq!(fleet["summary"]["machines"], 2);
  assert_eq!(fleet["summary"]["reachable"], 2);
  assert_eq!(fleet["summary"]["in_flight"], 2);
  a.stop();
  b.stop();
}

#[test]
fn fleet_slow_peers() {
  let temp = tempfile::tempdir().expect("temp root");
  let b_secrets = secrets_of(temp.path(), &[("TOOLU_EPIC_STATUS_TOKEN", "RIGHT-TOKEN")]);
  let mut b_config = config(free_port(), Vec::new(), b_secrets);
  b_config.addr = SocketAddr::from(([0, 0, 0, 0], b_config.addr.port()));
  let b = Engine::start(&temp.path().join("b"), "repo-500", "repo-501", b_config);
  let hole = blackhole();
  let peers = vec![
    peer("hole", hole.local_addr().expect("hole")),
    b.peer("server"),
  ];
  let a_secrets = secrets_of(
    temp.path(),
    &[("TOOLU_EPIC_PEER_TOKENS", r#"{"server":"WRONG"}"#)],
  );
  let a = Engine::start(
    &temp.path().join("a"),
    "repo-402",
    "repo-449",
    config(free_port(), peers, a_secrets),
  );
  let addr = a.addr;
  let started = Instant::now();
  let fleet = thread::spawn(move || get_json(addr, "/api/v1/fleet"));
  thread::sleep(Duration::from_millis(200));
  let local_started = Instant::now();
  let (status, _) = get(addr, "/api/v1/overview");
  assert_eq!(status, 200);
  assert!(local_started.elapsed() < Duration::from_millis(500));
  let fleet = fleet.join().expect("fleet request");
  assert!(
    started.elapsed() < Duration::from_secs(3),
    "{:?}",
    started.elapsed()
  );
  assert_eq!(machine(&fleet, "hole")["state"], "unreachable");
  let server = machine(&fleet, "server");
  assert_eq!(
    (server["state"].clone(), server["reason"].clone()),
    (json!("unauthorized"), json!("token"))
  );
  assert_eq!(keys(machine(&fleet, "local")), ["repo-449"]);
  a.stop();
  b.stop();
}

#[test]
fn fleet_token_canary_fleet() {
  let temp = tempfile::tempdir().expect("temp root");
  let canary = "PEER-TOKEN-CANARY-1";
  let echo = serve(200, "application/json", &overview(canary, canary));
  let plain = serve(200, "application/json", &overview("repo-7", "codex"));
  let tokens = json!({"echo": canary}).to_string();
  let secrets = secrets_of(temp.path(), &[("TOOLU_EPIC_PEER_TOKENS", &tokens)]);
  let peers = vec![echo.peer("echo"), plain.peer("plain")];
  let a = Engine::start(
    &temp.path().join("a"),
    "repo-402",
    "repo-449",
    config(free_port(), peers, secrets),
  );
  let (status, body) = get(a.addr, "/api/v1/fleet");
  assert_eq!(status, 200);
  assert!(!body.contains(canary), "{body}");
  assert!(body.contains("<redacted>"), "{body}");
  let head = echo.seen().to_ascii_lowercase();
  assert!(head.contains(&format!(
    "authorization: bearer {}",
    canary.to_ascii_lowercase()
  )));
  assert!(!plain.seen().to_ascii_lowercase().contains("authorization"));
  let root = a.root.clone();
  a.stop();
  assert!(!files_text(&root).contains(canary));
}

#[test]
fn fleet_last_seen() {
  let temp = tempfile::tempdir().expect("temp root");
  let b = Engine::start(
    &temp.path().join("b"),
    "repo-500",
    "repo-501",
    config(free_port(), Vec::new(), Secrets::none()),
  );
  let a_config = config(free_port(), vec![b.peer("server")], Secrets::none());
  let a = Engine::start(&temp.path().join("a"), "repo-402", "repo-449", a_config);
  let first = get_json(a.addr, "/api/v1/fleet");
  let seen = machine(&first, "server")["last_seen"].clone();
  assert!(seen.is_string(), "{first}");
  b.stop();
  let stopped = Instant::now();
  thread::sleep(Duration::from_millis(1_100));
  let second = get_json(a.addr, "/api/v1/fleet");
  let server = machine(&second, "server");
  assert_eq!(server["state"], "unreachable");
  assert_eq!(server["last_seen"], seen);
  let age = server["age_ms"].as_u64().expect("age");
  let elapsed = u64::try_from(stopped.elapsed().as_millis()).expect("elapsed");
  assert!(age >= elapsed, "{age} < {elapsed}");
  assert_eq!(server["in_flight"], json!([]));
  a.stop();
}

#[test]
fn fleet_refresher_first_round() {
  let temp = tempfile::tempdir().expect("temp root");
  let peer = serve(200, "application/json", &overview("repo-1", "claude"));
  let (running, stop) = listener(
    temp.path(),
    config(0, vec![peer.peer("server")], Secrets::none()),
  );
  let started = Instant::now();
  while peer.overview_requests() == 0 && started.elapsed() < Duration::from_secs(1) {
    thread::sleep(Duration::from_millis(10));
  }
  assert_eq!(peer.overview_requests(), 1);
  let quiet = tempfile::tempdir().expect("quiet root");
  let (idle, idle_stop) = listener(quiet.path(), config(0, Vec::new(), Secrets::none()));
  idle.stop(&idle_stop).expect("idle listener");
  running.stop(&stop).expect("listener thread");
}

fn concurrent_fleet(addr: SocketAddr) -> Vec<Duration> {
  let requests: Vec<_> = (0..5)
    .map(|_| {
      thread::spawn(move || {
        let started = Instant::now();
        let (status, _) = get(addr, "/api/v1/fleet");
        assert_eq!(status, 200);
        started.elapsed()
      })
    })
    .collect();
  requests
    .into_iter()
    .map(|request| request.join().expect("fleet request"))
    .collect()
}

#[test]
fn fleet_round_coalesces() {
  for delay in [Duration::ZERO, Duration::from_millis(1_500)] {
    let temp = tempfile::tempdir().expect("temp root");
    let peer = serve_after(
      delay,
      200,
      "application/json",
      &overview("repo-1", "claude"),
    );
    let (running, stop) = listener(
      temp.path(),
      config(0, vec![peer.peer("server")], Secrets::none()),
    );
    let first = get_json(running.addr, "/api/v1/fleet");
    assert_eq!(machine(&first, "server")["state"], "ok");
    assert_eq!(peer.overview_requests(), 1);
    thread::sleep(Duration::from_millis(1_100));
    let waits = concurrent_fleet(running.addr);
    assert_eq!(peer.overview_requests(), 2, "delay {delay:?}");
    assert!(
      waits
        .iter()
        .all(|wait| *wait < Duration::from_millis(2_500)),
      "{waits:?}"
    );
    running.stop(&stop).expect("listener thread");
  }
}
