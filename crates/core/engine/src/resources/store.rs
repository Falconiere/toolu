//! The persisted resource state and its policy (`packages/toolu-core/src/resources/resource-store.ts`):
//! `<home>/state.json` (`{version: 1, leases, cooldowns, pressure?}`), strict on
//! read, and `<home>/policy.json`. The state stays an `Ordered` document, so
//! a rewrite keeps every key TypeScript or a newer writer put there, in order.

use std::path::{Path, PathBuf};

use toolu_runtime::env::Env;
use toolu_runtime::json::ordered::Ordered;

use super::lock::{read_json_file, whole, with_resource_lock, write_json_atomic};
use super::pressure::is_pressure;

/// Machine capacity: agent and job slots, per-host agent caps, and whether
/// sustained pressure holds new work.
#[derive(Debug, Clone, PartialEq)]
pub struct ResourcePolicy {
  /// Agents at once (default 3).
  pub max_agents: i64,
  /// Expensive jobs at once (default 1).
  pub max_jobs: i64,
  /// Per-host agent caps.
  pub hosts: Vec<(String, i64)>,
  /// Whether a pressure hold refuses new work (default true).
  pub pressure: bool,
}

/// `resourceHome(env)`: `TOOLU_RESOURCE_HOME`, else `~/.local/state/toolu/resources`.
pub fn resource_home(env: &Env) -> PathBuf {
  env.get("TOOLU_RESOURCE_HOME").map_or_else(
    || env.home().join(".local/state/toolu/resources"),
    PathBuf::from,
  )
}

/// `Number.isSafeInteger`.
pub fn safe_integer(value: &Ordered) -> Option<i64> {
  let Ordered::Number(number) = value else {
    return None;
  };
  let whole = whole(number.as_f64()?)?;
  (whole.unsigned_abs() < (1_u64 << 53)).then_some(whole)
}

/// A capacity: a safe integer above zero.
fn capacity(value: &Ordered, name: &str) -> Result<i64, String> {
  safe_integer(value)
    .filter(|cap| *cap > 0)
    .ok_or_else(|| format!("invalid resource capacity {name}"))
}

/// `p.key ?? fallback`.
fn or_default<'a>(policy: &'a Ordered, key: &str, fallback: &'a Ordered) -> &'a Ordered {
  match policy.get(key) {
    None | Some(Ordered::Null) => fallback,
    Some(value) => value,
  }
}

/// The policy's `hosts`, each with a number for its capacity.
fn policy_hosts(policy: &Ordered) -> Result<Vec<(String, Ordered)>, String> {
  let entries = match policy.get("hosts") {
    None => return Ok(Vec::new()),
    Some(Ordered::Object(entries)) => entries,
    Some(
      Ordered::Null
      | Ordered::Bool(_)
      | Ordered::Number(_)
      | Ordered::String(_)
      | Ordered::Array(_),
    ) => return Err("invalid resource hosts".to_owned()),
  };
  let mut hosts = Vec::new();
  for (host, cap) in entries {
    if !matches!(cap, Ordered::Number(_)) {
      return Err(format!("invalid resource capacity {host}"));
    }
    hosts.push((host.clone(), cap.clone()));
  }
  Ok(hosts)
}

/// `resourcePolicy(root)`: `<root>/policy.json`, every key optional.
///
/// # Errors
/// A policy that is not an object, a non-object `hosts`, a non-number or
/// non-positive capacity, or a non-boolean `pressure`.
pub fn resource_policy(root: &Path) -> Result<ResourcePolicy, String> {
  let policy = read_json_file(&root.join("policy.json"), Ordered::Object(Vec::new()))?;
  let Ordered::Object(_) = policy else {
    return Err("invalid resource policy".to_owned());
  };
  let hosts = policy_hosts(&policy)?;
  let (three, one, yes) = (
    Ordered::Number(3.into()),
    Ordered::Number(1.into()),
    Ordered::Bool(true),
  );
  let (agents, jobs) = (
    or_default(&policy, "maxAgents", &three),
    or_default(&policy, "maxJobs", &one),
  );
  if !matches!(agents, Ordered::Number(_)) || !matches!(jobs, Ordered::Number(_)) {
    return Err("invalid resource capacity".to_owned());
  }
  let Ordered::Bool(pressure) = or_default(&policy, "pressure", &yes) else {
    return Err("invalid resource pressure policy".to_owned());
  };
  let max_agents = capacity(agents, "maxAgents")?;
  let max_jobs = capacity(jobs, "maxJobs")?;
  let hosts = hosts
    .iter()
    .map(|(host, cap)| capacity(cap, host).map(|cap| (host.clone(), cap)))
    .collect::<Result<_, _>>()?;
  Ok(ResourcePolicy {
    max_agents,
    max_jobs,
    hosts,
    pressure: *pressure,
  })
}

fn optional_string(lease: &Ordered, key: &str) -> bool {
  matches!(lease.get(key), None | Some(Ordered::String(_)))
}

fn positive_integer(value: Option<&Ordered>) -> bool {
  value.and_then(safe_integer).is_some_and(|n| n > 0)
}

/// `isLease`: the fields every lease needs, with their types.
pub(crate) fn is_lease(lease: &Ordered) -> bool {
  let string = |key: &str| matches!(lease.get(key), Some(Ordered::String(_)));
  matches!(lease, Ordered::Object(_))
    && matches!(lease.get("token"), Some(Ordered::String(token)) if !token.is_empty())
    && matches!(lease.get("type"), Some(Ordered::String(kind)) if kind == "agent" || kind == "job")
    && string("key")
    && string("stateDir")
    && ["host", "pendingHost", "worktree", "pane", "session"]
      .iter()
      .all(|key| optional_string(lease, key))
    && positive_integer(lease.get("ownerPid"))
    && (lease.get("groupPid").is_none() || positive_integer(lease.get("groupPid")))
    && string("stage")
    && string("createdAt")
    && string("heartbeatAt")
}

fn is_cooldown(value: &Ordered) -> bool {
  matches!(value, Ordered::Object(_))
    && matches!(value.get("until"), Some(Ordered::Number(n)) if n.as_f64().is_some_and(f64::is_finite))
    && matches!(value.get("reason"), Some(Ordered::String(_)))
}

/// `isResourceState`.
pub(crate) fn is_resource_state(state: &Ordered) -> bool {
  let version_one =
    matches!(state.get("version"), Some(Ordered::Number(n)) if n.as_f64() == Some(1.0));
  let leases =
    matches!(state.get("leases"), Some(Ordered::Array(items)) if items.iter().all(is_lease));
  let cooldowns = matches!(state.get("cooldowns"), Some(Ordered::Object(entries)) if entries.iter().all(|(_, cooldown)| is_cooldown(cooldown)));
  let pressure = state.get("pressure").is_none_or(is_pressure);
  matches!(state, Ordered::Object(_)) && version_one && leases && cooldowns && pressure
}

fn empty_state() -> Ordered {
  Ordered::Object(vec![
    ("version".to_owned(), Ordered::Number(1.into())),
    ("leases".to_owned(), Ordered::Array(Vec::new())),
    ("cooldowns".to_owned(), Ordered::Object(Vec::new())),
  ])
}

/// `readResourceState(root)`: `<root>/state.json`, an empty state when absent.
///
/// # Errors
/// A state that cannot be read or fails the strict schema.
pub fn read_resource_state(root: &Path) -> Result<Ordered, String> {
  let state = read_json_file(&root.join("state.json"), empty_state())?;
  if is_resource_state(&state) {
    Ok(state)
  } else {
    Err("invalid resource state; reconcile ownership before launching".to_owned())
  }
}

/// `updateResources(root, fn)`: `f` over the state under the lock. The state is
/// written back even when `f` fails, as TypeScript's `finally` does, and a
/// failed write wins over `f`'s result.
///
/// # Errors
/// A lock, read or write failure, or `f`'s error.
pub fn update_resources<T>(
  root: &Path,
  f: impl FnOnce(&mut Ordered) -> Result<T, String>,
) -> Result<T, String> {
  with_resource_lock(root, || {
    let mut state = read_resource_state(root)?;
    let result = f(&mut state);
    write_json_atomic(&root.join("state.json"), &state)?;
    result
  })
}

/// `state.leases`, mutably.
pub(crate) fn leases_mut(state: &mut Ordered) -> Option<&mut Vec<Ordered>> {
  match field_mut(state, "leases") {
    Some(Ordered::Array(items)) => Some(items),
    _ => None,
  }
}

/// `state.leases`.
pub fn leases(state: &Ordered) -> &[Ordered] {
  match state.get("leases") {
    Some(Ordered::Array(items)) => items,
    _ => &[],
  }
}

/// `object[key]`, mutably.
pub(crate) fn field_mut<'a>(object: &'a mut Ordered, key: &str) -> Option<&'a mut Ordered> {
  let Ordered::Object(entries) = object else {
    return None;
  };
  let (_, value) = entries.iter_mut().find(|(name, _)| name == key)?;
  Some(value)
}

#[cfg(test)]
#[path = "tests/store_test.rs"]
mod tests;
