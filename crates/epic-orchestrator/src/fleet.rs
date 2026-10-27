//! Read-only peer engine fetches and the fleet document that `/api/v1/fleet`
//! and `toolu epic status --fleet` share. The fleet document allow-lists peer
//! data field by field; every peer document is redacted with the local
//! secrets. A peer is never asked to act.

use std::collections::BTreeMap;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::{Map, Value, json};
use toolu_http::{Auth, Client, Method, Request};
use toolu_runtime::config::epic::Peer;
use toolu_runtime::config::secrets::{self, Secrets};
use toolu_runtime::env::Env;
use toolu_state::time::iso_millis;

/// The whole-request deadline of one peer call.
pub(crate) const PEER_TIMEOUT: Duration = Duration::from_secs(2);
const BODY_LIMIT: usize = 1024 * 1024;
const TEXT_LIMIT: usize = 160;

/// What a peer's reply says about it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PeerState {
  /// A schema-1 document arrived.
  Ok,
  /// No usable reply: connect error, timeout, redirect or server error.
  Unreachable,
  /// The peer refused the token (`401`).
  TokenRejected,
  /// The peer engine refused the request `Host` (`403 foreign_host`).
  HostRejected,
  /// Some other `403`, such as a proxy in front of the peer.
  Refused,
  /// The peer speaks another status API.
  VersionMismatch,
}

impl PeerState {
  /// The state's name in the fleet document.
  pub(crate) fn name(self) -> &'static str {
    match self {
      Self::Ok => "ok",
      Self::Unreachable => "unreachable",
      Self::TokenRejected | Self::HostRejected | Self::Refused => "unauthorized",
      Self::VersionMismatch => "version-mismatch",
    }
  }

  /// Which credential an `unauthorized` peer refused.
  pub(crate) fn reason(self) -> Option<&'static str> {
    match self {
      Self::TokenRejected => Some("token"),
      Self::HostRejected => Some("host"),
      Self::Ok | Self::Unreachable | Self::Refused | Self::VersionMismatch => None,
    }
  }
}

/// One peer call's outcome.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Fetched {
  /// The peer's schema-1 JSON document.
  Doc(Value),
  /// The peer has no such document.
  NotFound,
  /// The peer could not answer usefully.
  Failed(PeerState),
}

/// One machine of the fleet before projection.
#[derive(Clone)]
pub(crate) struct Machine {
  pub name: String,
  pub local: bool,
  pub state: PeerState,
  pub last_seen_ms: Option<u64>,
  pub overview: Option<Value>,
  /// A peer's own dashboard: its configured origin, which config validation
  /// keeps free of credentials, path, query and fragment. `None` for local.
  pub dashboard_url: Option<String>,
}

/// A peer client: short deadline, bounded body, no redirect, no proxy.
///
/// # Errors
/// The HTTP client rejects its configuration.
pub(crate) fn client() -> Result<Client, String> {
  let config = toolu_http::Config {
    timeout: PEER_TIMEOUT,
    max_body_bytes: BODY_LIMIT,
    max_redirects: 0,
    test_root_ca_der: None,
  };
  Client::new(config, &Env::default()).map_err(|err| err.to_string())
}

/// `GET {peer}/api/v1/{route}` with the peer's token, if one is configured.
pub(crate) fn fetch(client: &Client, peer: &Peer, token: Option<&str>, route: &str) -> Fetched {
  let auth = token.map_or(Auth::None, |token| Auth::Bearer(token.to_owned()));
  let url = format!("{}api/v1/{route}", peer.url);
  let request = Request {
    method: Method::Get,
    url: &url,
    auth: &auth,
    headers: &[("accept", "application/json")],
    body: None,
  };
  match client.send(&request) {
    Ok(response) => classify(response.status, &response.body, route == "overview"),
    Err(_err) => Fetched::Failed(PeerState::Unreachable),
  }
}

/// A reply's status and body as a fetch outcome.
pub(crate) fn classify(status: u16, body: &[u8], overview: bool) -> Fetched {
  match status {
    200 => match serde_json::from_slice::<Value>(body) {
      Ok(value) if value.get("schema").and_then(Value::as_u64) == Some(1) => Fetched::Doc(value),
      Ok(_) | Err(_) => Fetched::Failed(PeerState::VersionMismatch),
    },
    404 if overview => Fetched::Failed(PeerState::VersionMismatch),
    404 => Fetched::NotFound,
    401 => Fetched::Failed(PeerState::TokenRejected),
    403 => {
      let error = serde_json::from_slice::<Value>(body).ok();
      let code = error.as_ref().and_then(|value| value.get("error"));
      if code.and_then(Value::as_str) == Some("foreign_host") {
        Fetched::Failed(PeerState::HostRejected)
      } else {
        Fetched::Failed(PeerState::Refused)
      }
    }
    _ => Fetched::Failed(PeerState::Unreachable),
  }
}

/// A configured peer and its token. No `Debug`: the token must never print.
pub(crate) struct Target {
  pub peer: Peer,
  pub token: Option<String>,
}

/// Pair each configured peer with its resolved token.
pub(crate) fn targets(peers: &[Peer], secrets: &Secrets) -> Vec<Target> {
  peers
    .iter()
    .map(|peer| Target {
      peer: peer.clone(),
      token: secrets.peer_token(&peer.name).map(str::to_owned),
    })
    .collect()
}

/// Every peer's overview, fetched in parallel; the wait is the slowest peer.
pub(crate) fn fetch_all(client: &Client, targets: &[Target]) -> Vec<Fetched> {
  thread::scope(|scope| {
    let handles: Vec<_> = targets
      .iter()
      .map(|target| {
        scope.spawn(move || fetch(client, &target.peer, target.token.as_deref(), "overview"))
      })
      .collect();
    handles
      .into_iter()
      .map(|handle| {
        // A panicked peer thread is a peer that gave no usable answer.
        // `Result::unwrap_or` never panics: `Err` yields the default.
        handle
          .join()
          .unwrap_or(Fetched::Failed(PeerState::Unreachable))
      })
      .collect()
  })
}

/// The peer machines for fetched overviews; `last_seen` keeps each peer's
/// previous success and is updated in place for this round's.
pub(crate) fn peer_machines(
  targets: &[Target],
  fetched: Vec<Fetched>,
  last_seen: &mut BTreeMap<String, u64>,
  now_ms: u64,
) -> Vec<Machine> {
  targets
    .iter()
    .zip(fetched)
    .map(|(Target { peer, .. }, fetched)| {
      let (state, overview) = match fetched {
        Fetched::Doc(value) => (PeerState::Ok, Some(value)),
        Fetched::NotFound => (PeerState::VersionMismatch, None),
        Fetched::Failed(state) => (state, None),
      };
      if state == PeerState::Ok {
        last_seen.insert(peer.name.clone(), now_ms);
      }
      Machine {
        name: peer.name.clone(),
        local: false,
        state,
        last_seen_ms: last_seen.get(&peer.name).copied(),
        overview,
        dashboard_url: Some(peer.url.clone()),
      }
    })
    .collect()
}

/// The local machine for this engine's overview document.
pub(crate) fn local_machine(overview: Value, now_ms: u64) -> Machine {
  Machine {
    name: "local".to_owned(),
    local: true,
    state: PeerState::Ok,
    last_seen_ms: Some(now_ms),
    overview: Some(overview),
    dashboard_url: None,
  }
}

/// The versioned fleet document, redacted with the local secrets.
pub(crate) fn document(machines: &[Machine], now_ms: u64, secrets: &Secrets) -> Value {
  let projected: Vec<Value> = machines
    .iter()
    .map(|machine| project(machine, now_ms))
    .collect();
  let value = json!({"schema": 1, "summary": summary(&projected), "machines": projected});
  secrets::redact_json(&value, secrets)
}

fn project(machine: &Machine, now_ms: u64) -> Value {
  let overview = machine
    .overview
    .as_ref()
    .filter(|_| machine.state == PeerState::Ok);
  let empty = Value::Null;
  let doc = overview.unwrap_or(&empty);
  let last_seen = machine
    .last_seen_ms
    .map(|ms| iso_millis(UNIX_EPOCH + Duration::from_millis(ms)));
  json!({
    "name": machine.name,
    "local": machine.local,
    "state": machine.state.name(),
    "reason": machine.state.reason(),
    "hostname": overview.and_then(|doc| doc.get("hostname")).filter(|name| name.as_str().is_some_and(|name| !name.is_empty())).map(|name| text(Some(name))),
    "dashboard_url": machine.dashboard_url,
    "age_ms": machine.last_seen_ms.map(|ms| now_ms.saturating_sub(ms)),
    "last_seen": last_seen,
    "engine": overview.map(|doc| text(doc.get("engine"))),
    "paused": overview.map(|doc| doc.get("paused").and_then(Value::as_bool).unwrap_or(false)),
    "journal_seq": overview.map(|doc| number(doc.get("journal_seq"))),
    "epics": list(doc.get("epics")).iter().map(epic_row).collect::<Vec<_>>(),
    "in_flight": list(doc.get("issues")).iter().filter_map(issue_row).collect::<Vec<_>>(),
    "hosts": list(doc.get("hosts").and_then(|hosts| hosts.get("items")))
      .iter()
      .map(host_row)
      .collect::<Vec<_>>(),
    "counts": counts(doc.get("counts")),
    "attention_count": number(doc.get("attention_count")),
  })
}

fn summary(machines: &[Value]) -> Value {
  let ok: Vec<&Value> = machines
    .iter()
    .filter(|machine| machine.get("state").and_then(Value::as_str) == Some("ok"))
    .collect();
  let mut hosts = BTreeMap::<String, (u64, u64)>::new();
  for row in ok.iter().flat_map(|machine| list(machine.get("hosts"))) {
    let entry = hosts.entry(text(row.get("host"))).or_default();
    entry.0 = entry.0.saturating_add(number(row.get("running")));
    entry.1 = entry.1.saturating_add(number(row.get("cap")));
  }
  let sum = |field: &str| -> u64 {
    ok.iter()
      .map(|machine| match machine.get(field) {
        Some(Value::Array(items)) => items
          .iter()
          .fold(0, |count: u64, _| count.saturating_add(1)),
        other => number(other),
      })
      .fold(0, u64::saturating_add)
  };
  json!({
    "machines": machines.len(),
    "reachable": ok.len(),
    "hosts": hosts
      .into_iter()
      .map(|(host, (running, cap))| json!({"host": host, "running": running, "cap": cap}))
      .collect::<Vec<_>>(),
    "in_flight": sum("in_flight"),
    "attention_count": sum("attention_count"),
  })
}

fn epic_row(row: &Value) -> Value {
  json!({
    "key": text(row.get("key")),
    "paused": row.get("paused").and_then(Value::as_bool).unwrap_or(false),
    "counts": counts(row.get("counts")),
    "attention_count": number(row.get("attention_count")),
  })
}

fn issue_row(row: &Value) -> Option<Value> {
  let stage = text(row.get("stage"));
  matches!(stage.as_str(), "running" | "awaiting_merge").then(|| {
    json!({
      "key": text(row.get("key")),
      "epic": text(row.get("epic")),
      "stage": stage,
      "phase": text(row.get("phase")),
    })
  })
}

fn host_row(row: &Value) -> Value {
  json!({
    "host": text(row.get("host")),
    "running": number(row.get("running")),
    "cap": number(row.get("cap")),
  })
}

fn list(value: Option<&Value>) -> &[Value] {
  value.and_then(Value::as_array).map_or(&[], Vec::as_slice)
}

fn counts(value: Option<&Value>) -> Value {
  let map: Map<String, Value> = value
    .and_then(Value::as_object)
    .into_iter()
    .flatten()
    .filter_map(|(key, count)| {
      count
        .as_u64()
        .map(|count| (key.chars().take(TEXT_LIMIT).collect(), json!(count)))
    })
    .collect();
  Value::Object(map)
}

fn text(value: Option<&Value>) -> String {
  value
    .and_then(Value::as_str)
    .unwrap_or("")
    .chars()
    .take(TEXT_LIMIT)
    .collect()
}

fn number(value: Option<&Value>) -> u64 {
  value.and_then(Value::as_u64).unwrap_or(0)
}

/// Milliseconds since the epoch now.
pub(crate) fn now_ms() -> u64 {
  toolu_state::time::epoch_millis(SystemTime::now())
}

#[cfg(test)]
#[path = "tests/fleet_test.rs"]
mod tests;
