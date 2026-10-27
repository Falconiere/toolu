use std::collections::BTreeMap;

use serde_json::{Value, json};
use toolu_runtime::config::secrets::Secrets;

use super::{Fetched, Machine, PeerState, Target, classify, client, document, fetch, fetch_all};
use crate::peer_fixture::{blackhole, closed, peer, serve};

fn overview(issue: &str, host: &str) -> Value {
  json!({
    "schema": 1, "engine": "running", "paused": false, "journal_seq": 4,
    "epics": [{"key": "repo-402", "paused": false, "counts": {"running": 1}, "attention_count": 1}],
    "issues": [
      {"key": issue, "epic": "repo-402", "stage": "running", "phase": "execution"},
      {"key": "done-1", "epic": "repo-402", "stage": "merged", "phase": "ready"}
    ],
    "hosts": {"schema": 1, "items": [{"host": host, "running": 1, "cap": 3}]},
    "counts": {"running": 1, "merged": 1}, "attention_count": 1,
    "machine": {"agent_leases": [{"worktree": "/private/worktree"}]}
  })
}

fn machine(name: &str, state: PeerState, doc: Option<Value>) -> Machine {
  Machine {
    name: name.to_owned(),
    local: name == "local",
    state,
    last_seen_ms: doc.as_ref().map(|_| 1_000),
    overview: doc,
    dashboard_url: None,
  }
}

fn failed(status: u16, body: &[u8], overview: bool) -> Option<PeerState> {
  match classify(status, body, overview) {
    Fetched::Failed(state) => Some(state),
    Fetched::Doc(_) | Fetched::NotFound => None,
  }
}

#[test]
fn fleet_classifies_replies() {
  let ok = classify(200, br#"{"schema":1,"x":1}"#, true);
  assert!(matches!(ok, Fetched::Doc(value) if value["x"] == 1));
  for body in [
    &br#"{"schema":2}"#[..],
    b"not json",
    b"[1]",
    br#"{"engine":"running"}"#,
  ] {
    assert_eq!(failed(200, body, true), Some(PeerState::VersionMismatch));
  }
  assert_eq!(failed(404, b"", true), Some(PeerState::VersionMismatch));
  assert_eq!(classify(404, b"", false), Fetched::NotFound);
  for status in [302, 500, 503] {
    assert_eq!(failed(status, b"", true), Some(PeerState::Unreachable));
  }
}

#[test]
fn fleet_classifies_refusals() {
  assert_eq!(failed(401, b"", true), Some(PeerState::TokenRejected));
  let foreign = br#"{"schema":1,"error":"foreign_host"}"#;
  assert_eq!(failed(403, foreign, true), Some(PeerState::HostRejected));
  assert_eq!(
    failed(403, b"<html>denied</html>", true),
    Some(PeerState::Refused)
  );
  assert_eq!(PeerState::Refused.name(), "unauthorized");
  assert_eq!(PeerState::Refused.reason(), None);
  assert_eq!(PeerState::HostRejected.reason(), Some("host"));
  assert_eq!(PeerState::TokenRejected.reason(), Some("token"));
}

fn state_of(fetched: &Fetched) -> Option<PeerState> {
  match fetched {
    Fetched::Doc(_) => Some(PeerState::Ok),
    Fetched::Failed(state) => Some(*state),
    Fetched::NotFound => None,
  }
}

#[test]
fn fleet_peer_states_over_http() {
  let big = format!(r#"{{"schema":1,"pad":"{}"}}"#, "x".repeat(1_500_000));
  let ok = overview("repo-1", "claude").to_string();
  let json = "application/json";
  let cases = [
    (200, json, ok.as_str(), PeerState::Ok),
    (200, json, r#"{"schema":2}"#, PeerState::VersionMismatch),
    (200, "text/plain", "hello", PeerState::VersionMismatch),
    (404, json, r#"{"schema":1}"#, PeerState::VersionMismatch),
    (401, json, r#"{"schema":1}"#, PeerState::TokenRejected),
    (
      403,
      json,
      r#"{"error":"foreign_host"}"#,
      PeerState::HostRejected,
    ),
    (302, "text/plain", "", PeerState::Unreachable),
    (200, json, big.as_str(), PeerState::Unreachable),
  ];
  let servers: Vec<_> = cases
    .iter()
    .map(|(status, kind, body, _)| serve(*status, kind, body))
    .collect();
  let mut targets: Vec<Target> = servers
    .iter()
    .enumerate()
    .map(|(index, server)| Target {
      peer: server.peer(&format!("p{index}")),
      token: (index == 0).then(|| "TOKEN-ZERO".to_owned()),
    })
    .collect();
  targets.push(Target {
    peer: peer("closed", closed()),
    token: None,
  });
  let client = client().expect("client");
  let states: Vec<_> = fetch_all(&client, &targets).iter().map(state_of).collect();
  let mut expected: Vec<_> = cases.iter().map(|case| Some(case.3)).collect();
  expected.push(Some(PeerState::Unreachable));
  assert_eq!(states, expected);
  let head = servers[0].seen().to_ascii_lowercase();
  assert!(head.starts_with("get /api/v1/overview http/1.1"), "{head}");
  assert!(head.contains("authorization: bearer token-zero"), "{head}");
  assert!(
    !servers[1]
      .seen()
      .to_ascii_lowercase()
      .contains("authorization")
  );
  let detail = fetch(&client, &servers[3].peer("p3"), None, "issues/repo-1");
  assert_eq!(detail, Fetched::NotFound);
}

fn assert_hostile(entry: &Value) {
  let keys: Vec<&String> = entry.as_object().expect("machine").keys().collect();
  let allowed = [
    "age_ms",
    "attention_count",
    "counts",
    "dashboard_url",
    "engine",
    "epics",
    "hostname",
    "hosts",
    "in_flight",
    "journal_seq",
    "last_seen",
    "local",
    "name",
    "paused",
    "reason",
    "state",
  ];
  assert_eq!(keys, allowed.iter().collect::<Vec<_>>());
  assert_eq!(entry["engine"], "");
  assert_eq!(entry["hostname"], Value::Null);
  assert_eq!(entry["paused"], false);
  assert_eq!(entry["journal_seq"], 0);
  assert_eq!(entry["attention_count"], 0);
  assert_eq!(entry["counts"], json!({"awaiting_merge": 2}));
  assert_eq!(entry["in_flight"], json!([]));
  assert_eq!(
    entry["epics"][0],
    json!({"key": "repo-9", "paused": false,
    "counts": {"awaiting_merge": 2}, "attention_count": 0})
  );
  assert_eq!(
    entry["hosts"][0],
    json!({"host": "codex", "running": 0, "cap": 4})
  );
  assert_eq!(entry["age_ms"], 1_000);
  assert_eq!(entry["last_seen"], "1970-01-01T00:00:01.000Z");
}

#[test]
fn fleet_projection_drops_hostile_fields() {
  let hostile = json!({
    "schema": 1, "engine": 7, "paused": "yes", "journal_seq": "nine", "secret": "drop-me",
    "hostname": {"name": "drop-me"},
    "epics": [{"key": "repo-9", "counts": {"running": "1", "awaiting_merge": 2}, "extra": 1}],
    "issues": {"key": "not-a-list"},
    "hosts": {"items": [{"host": "codex", "running": "2", "cap": 4, "token": "x"}]},
    "counts": {"running": -1, "awaiting_merge": 2}, "attention_count": "many"
  });
  let mut long = hostile.clone();
  long["issues"] = json!([{"key": "repo-10", "stage": "running", "phase": "p".repeat(300)}]);
  long["hostname"] = json!("h".repeat(300));
  let mut blank = long.clone();
  blank["hostname"] = json!("");
  let doc = document(
    &[
      machine("hostile", PeerState::Ok, Some(hostile)),
      machine("long", PeerState::Ok, Some(long)),
      machine("blank", PeerState::Ok, Some(blank)),
    ],
    2_000,
    &Secrets::none(),
  );
  assert_hostile(&doc["machines"][0]);
  let phase = doc["machines"][1]["in_flight"][0]["phase"]
    .as_str()
    .expect("phase");
  assert_eq!(phase.chars().count(), 160);
  assert_eq!(doc["machines"][1]["hostname"], json!("h".repeat(160)));
  assert_eq!(doc["machines"][2]["hostname"], Value::Null);
  assert!(!doc.to_string().contains("drop-me"));
}

#[test]
fn fleet_summary_sums_ok_machines() {
  let mut seen = BTreeMap::new();
  let targets = [
    Target {
      peer: peer("server", closed()),
      token: Some("PEER-CANARY".to_owned()),
    },
    Target {
      peer: peer("nas", closed()),
      token: None,
    },
  ];
  let peers = super::peer_machines(
    &targets,
    vec![
      Fetched::Doc(overview("repo-2", "PEER-CANARY")),
      Fetched::Failed(PeerState::Unreachable),
    ],
    &mut seen,
    5_000,
  );
  assert_eq!(seen.get("server"), Some(&5_000));
  let secrets = toolu_runtime::config::secrets::load(&toolu_runtime::host::roots::Roots::new(
    toolu_runtime::env::Env::from_pairs([
      ("HOME", "/nonexistent-fleet-home"),
      ("TOOLU_EPIC_PEER_TOKENS", r#"{"server":"PEER-CANARY"}"#),
    ]),
    None,
  ))
  .expect("secrets");
  let mut machines = vec![super::local_machine(overview("repo-1", "claude"), 6_000)];
  machines.extend(peers);
  let doc = document(&machines, 6_000, &secrets);
  assert_eq!(doc["schema"], 1);
  assert_eq!(doc["summary"]["machines"], 3);
  assert_eq!(doc["summary"]["reachable"], 2);
  assert_eq!(doc["summary"]["in_flight"], 2);
  assert_eq!(doc["summary"]["attention_count"], 2);
  assert_eq!(doc["summary"]["hosts"][0]["host"], "<redacted>");
  assert_eq!(
    doc["summary"]["hosts"][1],
    json!({"host": "claude", "running": 1, "cap": 3})
  );
  let nas = &doc["machines"][2];
  assert_eq!(nas["state"], "unreachable");
  assert_eq!(nas["last_seen"], Value::Null);
  assert_eq!(nas["engine"], Value::Null);
  assert_eq!(nas["counts"], json!({}));
  assert_eq!(doc["machines"][1]["age_ms"], 1_000);
  assert!(!doc.to_string().contains("PEER-CANARY"));
}

#[test]
fn fleet_without_peers_lists_local() {
  let down = crate::status::document(&crate::model::World::new(0), false, None);
  let doc = document(&[super::local_machine(down, 9)], 9, &Secrets::none());
  assert_eq!(doc["summary"]["machines"], 1);
  let local = &doc["machines"][0];
  assert_eq!(local["name"], "local");
  assert_eq!(local["local"], true);
  assert_eq!(local["state"], "ok");
  assert_eq!(local["engine"], "down");
  assert_eq!(local["age_ms"], 0);
  assert_eq!(local["in_flight"], json!([]));
  assert_eq!(local["journal_seq"], 0);
}

#[test]
fn fleet_blackholed_peer_ends_at_the_deadline() {
  let hole = blackhole();
  let target = peer("hole", hole.local_addr().expect("hole address"));
  let started = std::time::Instant::now();
  let fetched = fetch(&client().expect("client"), &target, None, "overview");
  let waited = started.elapsed();
  assert_eq!(fetched, Fetched::Failed(PeerState::Unreachable));
  assert!(waited >= super::PEER_TIMEOUT, "{waited:?}");
  assert!(waited < std::time::Duration::from_secs(3), "{waited:?}");
}
