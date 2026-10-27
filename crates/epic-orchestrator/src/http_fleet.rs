//! `/api/v1/fleet` and the read-only peer proxy of the status listener. One
//! peer round runs at a time and is shared; a refresher keeps last-seen times
//! while no page is open. The local machine is never cached.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use toolu_http::Client;
use toolu_runtime::config::epic::Peer;
use toolu_runtime::config::secrets::{self, Secrets};

use crate::fleet::{self, Fetched, Machine, PeerState, Target};
use crate::http_response::{Response, json_response};
use crate::model::World;
use crate::paths::Paths;
use crate::status_api;

/// How old a shared round may be for a `/api/v1/fleet` request.
const REQUEST_AGE: Duration = Duration::from_secs(1);
/// The refresher's period; it accepts a round up to 5/6 of it old.
pub(crate) const REFRESH_EVERY: Duration = Duration::from_secs(30);
/// Proxied peer calls in flight at once.
const PROXIES: usize = 8;
/// Detail routes a peer's documents are proxied for.
const ROUTES: [&str; 8] = [
  "overview",
  "epics",
  "agents",
  "hosts",
  "machine",
  "jobs",
  "queue",
  "attention",
];

/// Configured peers, their client and the last shared round.
pub(crate) struct FleetCache {
  targets: Vec<Target>,
  client: Option<Client>,
  round: Mutex<Round>,
  proxies: AtomicUsize,
}

#[derive(Default)]
struct Round {
  at: Option<Instant>,
  machines: Vec<Machine>,
  last_seen: BTreeMap<String, u64>,
}

impl FleetCache {
  /// The cache for `peers`, their tokens taken from `secrets`.
  pub(crate) fn new(peers: &[Peer], secrets: &Secrets) -> Self {
    Self {
      targets: fleet::targets(peers, secrets),
      client: fleet::client().ok(),
      round: Mutex::new(Round::default()),
      proxies: AtomicUsize::new(0),
    }
  }

  /// The peer machines of a round no older than `max_age`, running one if needed.
  ///
  /// # Errors
  /// The round mutex is poisoned.
  pub(crate) fn refresh(&self, max_age: Duration) -> Result<Vec<Machine>, ()> {
    let mut round = self.round.lock().map_err(|_err| ())?;
    if round.at.is_none_or(|at| at.elapsed() >= max_age) {
      let fetched = match &self.client {
        Some(client) => fleet::fetch_all(client, &self.targets),
        None => vec![Fetched::Failed(PeerState::Unreachable); self.targets.len()],
      };
      let round = &mut *round;
      round.machines = fleet::peer_machines(
        &self.targets,
        fetched,
        &mut round.last_seen,
        fleet::now_ms(),
      );
      round.at = Some(Instant::now());
    }
    Ok(round.machines.clone())
  }
}

/// The background round: at start, then every 30 s, only while peers exist.
pub(crate) struct Refresher(Option<JoinHandle<()>>);

impl Refresher {
  /// Start the first round now, then one every `period`; `stop` and
  /// [`Refresher::stop`] end the loop.
  pub(crate) fn spawn(fleet: &Arc<FleetCache>, stop: &Arc<AtomicBool>, period: Duration) -> Self {
    if fleet.targets.is_empty() {
      return Self(None);
    }
    let fleet = Arc::clone(fleet);
    let stop = Arc::clone(stop);
    Self(Some(thread::spawn(move || {
      let max_age = period.mul_f64(5.0 / 6.0);
      while !stop.load(Ordering::Relaxed) && fleet.refresh(max_age).is_ok() {
        thread::park_timeout(period);
      }
    })))
  }

  /// Wake the loop after `stop` was raised, and join it.
  pub(crate) fn stop(self) -> thread::Result<()> {
    match self.0 {
      Some(handle) => {
        handle.thread().unpark();
        handle.join()
      }
      None => Ok(()),
    }
  }
}

/// A fleet route's response, or `None` for any other path.
pub(crate) fn response(
  fleet: &FleetCache,
  world: &Mutex<World>,
  paths: &Paths,
  secrets: &Secrets,
  path: &str,
) -> Option<Response> {
  if path == "/api/v1/fleet" {
    return Some(document(fleet, world, paths, secrets));
  }
  let rest = path.strip_prefix("/api/v1/fleet/")?;
  Some(
    rest
      .strip_prefix("peers/")
      .map_or_else(not_found, |rest| proxy(fleet, rest, secrets)),
  )
}

fn document(
  fleet: &FleetCache,
  world: &Mutex<World>,
  paths: &Paths,
  secrets: &Secrets,
) -> Response {
  let snapshot = match world.lock() {
    Ok(world) => world.clone(),
    Err(_err) => return Response::error(500, "status_unavailable"),
  };
  let Ok(local) = status_api::document(&snapshot, paths, None, secrets) else {
    return Response::error(500, "status_unavailable");
  };
  let Ok(peers) = fleet.refresh(REQUEST_AGE) else {
    return Response::error(500, "status_unavailable");
  };
  let now = fleet::now_ms();
  let mut machines = vec![fleet::local_machine(local, now)];
  machines.extend(peers);
  json_response(Ok(Some(fleet::document(&machines, now, secrets))))
}

fn proxy(fleet: &FleetCache, rest: &str, secrets: &Secrets) -> Response {
  let Some((name, route)) = rest.split_once('/') else {
    return not_found();
  };
  let target =
    decode(name).and_then(|name| fleet.targets.iter().find(|target| target.peer.name == name));
  let (Some(target), Some(client)) = (target, fleet.client.as_ref()) else {
    return not_found();
  };
  if !route_allowed(route) {
    return not_found();
  }
  if fleet.proxies.fetch_add(1, Ordering::AcqRel) >= PROXIES {
    fleet.proxies.fetch_sub(1, Ordering::AcqRel);
    return Response::error(503, "proxy_limit");
  }
  let fetched = fleet::fetch(client, &target.peer, target.token.as_deref(), route);
  fleet.proxies.fetch_sub(1, Ordering::AcqRel);
  match fetched {
    Fetched::Doc(value) => json_response(Ok(Some(secrets::redact_json(&value, secrets)))),
    Fetched::NotFound => not_found(),
    Fetched::Failed(PeerState::Unreachable | PeerState::Ok) => {
      Response::error(502, "peer_unreachable")
    }
    Fetched::Failed(PeerState::TokenRejected | PeerState::HostRejected | PeerState::Refused) => {
      Response::error(502, "peer_unauthorized")
    }
    Fetched::Failed(PeerState::VersionMismatch) => Response::error(502, "peer_version_mismatch"),
  }
}

fn not_found() -> Response {
  Response::error(404, "not_found")
}

/// Whether `route` is a proxied document: a fixed route, or an epic or
/// issue key of `[A-Za-z0-9._-]{1,128}` other than `.` and `..`.
fn route_allowed(route: &str) -> bool {
  if ROUTES.contains(&route) {
    return true;
  }
  let Some((kind, key)) = route.split_once('/') else {
    return false;
  };
  matches!(kind, "epics" | "issues")
    && (1..=128).contains(&key.len())
    && key != "."
    && key != ".."
    && key
      .bytes()
      .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

/// `%XX` escapes to bytes, then UTF-8; any malformed escape is `None`.
fn decode(text: &str) -> Option<String> {
  let mut bytes = Vec::with_capacity(text.len());
  let mut rest = text.as_bytes();
  while let Some((&byte, tail)) = rest.split_first() {
    if byte == b'%' {
      let hex = tail.get(..2)?;
      let pair = std::str::from_utf8(hex).ok()?;
      bytes.push(u8::from_str_radix(pair, 16).ok()?);
      rest = tail.get(2..)?;
    } else {
      bytes.push(byte);
      rest = tail;
    }
  }
  String::from_utf8(bytes).ok()
}

#[cfg(test)]
#[path = "tests/http_fleet_test.rs"]
mod tests;

#[cfg(test)]
#[path = "tests/http_fleet_proxy_test.rs"]
mod proxy_tests;

#[cfg(test)]
#[path = "tests/http_fleet_board_test.rs"]
mod board_tests;
