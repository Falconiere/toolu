//! Host pools, cooldowns and host choice, plus the strict loaders launch uses
//! for a saved pool and route (`launch-route.ts`): a malformed file never
//! becomes a default-host launch.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde_json::Value;

use crate::hosts::{HOST_KINDS, HostKind, host_kind};

/// Hosts and their caps, in preference order for ties.
pub(crate) type HostPool = Vec<(HostKind, u64)>;

/// `claude:2,codex` → a pool; a bare host gets `default_cap`.
///
/// # Errors
/// An unknown host, a cap that is not a positive whole number, a repeated
/// host, or an empty pool.
pub(crate) fn parse_hosts(spec: &str, default_cap: u64) -> Result<HostPool, String> {
  let mut pool: HostPool = Vec::new();
  for part in spec
    .split(',')
    .map(str::trim)
    .filter(|part| !part.is_empty())
  {
    let mut fields = part.split(':');
    let kind = host_kind(fields.next().unwrap_or(""))?;
    let cap = match fields.next() {
      None | Some("") => Some(default_cap),
      Some(raw) => raw.parse::<u64>().ok(),
    };
    let duplicate = pool.iter().any(|(seen, _)| *seen == kind);
    let extra = fields.next().is_some();
    let Some(cap) = cap.filter(|cap| *cap > 0 && !extra && !duplicate) else {
      return Err(format!("invalid host capacity: {part}"));
    };
    pool.push((kind, cap));
  }
  if pool.is_empty() {
    return Err("host pool is empty".to_owned());
  }
  Ok(pool)
}

/// Hosts whose `until` (ISO time or epoch milliseconds) is still ahead.
pub(crate) fn cooling_hosts(cooldowns: &Value, now_ms: u64) -> BTreeSet<HostKind> {
  HOST_KINDS
    .into_iter()
    .filter(|kind| {
      let until = cooldowns
        .get(kind.name())
        .and_then(|entry| entry.get("until"));
      until.and_then(until_ms).is_some_and(|until| until > now_ms)
    })
    .collect()
}

fn until_ms(value: &Value) -> Option<u64> {
  match value {
    Value::String(text) => toolu_state::time::parse_iso(text).map(toolu_state::time::epoch_millis),
    Value::Number(number) => number.as_u64().or_else(|| {
      number
        .as_f64()
        .and_then(|ms| format!("{:.0}", ms.max(0.0)).parse().ok())
    }),
    Value::Null | Value::Bool(_) | Value::Array(_) | Value::Object(_) => None,
  }
}

/// The open host with the most free capacity; a preferred host wins when one
/// has room; ties keep pool order.
pub(crate) fn pick_host(
  pool: &HostPool,
  used: &BTreeMap<HostKind, u64>,
  cooling: &BTreeSet<HostKind>,
  prefer: &[HostKind],
) -> Option<HostKind> {
  let free = |kind: &HostKind, cap: u64| cap.saturating_sub(used.get(kind).copied().unwrap_or(0));
  let open: Vec<(HostKind, u64)> = pool
    .iter()
    .filter(|(kind, cap)| !cooling.contains(kind) && free(kind, *cap) > 0)
    .copied()
    .collect();
  let preferred: Vec<(HostKind, u64)> = open
    .iter()
    .filter(|(kind, _)| prefer.contains(kind))
    .copied()
    .collect();
  let candidates = if preferred.is_empty() {
    open
  } else {
    preferred
  };
  let mut best: Option<(HostKind, u64)> = None;
  for (kind, cap) in candidates {
    let room = free(&kind, cap);
    if best.is_none_or(|(_, most)| room > most) {
      best = Some((kind, room));
    }
  }
  best.map(|(kind, _)| kind)
}

fn read_strict(path: &Path) -> Result<Option<Value>, String> {
  let text = match std::fs::read_to_string(path) {
    Ok(text) => text,
    Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
    Err(err) => return Err(format!("{}: {err}", path.display())),
  };
  serde_json::from_str(&text)
    .map(Some)
    .map_err(|err| format!("invalid {}: malformed JSON: {err}", path.display()))
}

/// The saved pool, or `fallback` when none is saved.
///
/// # Errors
/// A malformed pool: not a non-empty list of known hosts with positive caps,
/// or a repeated host.
pub(crate) fn load_host_pool(path: &Path, fallback: HostPool) -> Result<HostPool, String> {
  let Some(value) = read_strict(path)? else {
    return Ok(fallback);
  };
  let bad = || format!("invalid host pool {}", path.display());
  let rows = value
    .as_array()
    .filter(|rows| !rows.is_empty())
    .ok_or_else(bad)?;
  let mut pool = HostPool::new();
  for row in rows {
    let kind = row
      .get("kind")
      .and_then(Value::as_str)
      .and_then(saved_host)
      .ok_or_else(bad)?;
    let cap = row
      .get("cap")
      .and_then(Value::as_u64)
      .filter(|cap| *cap > 0)
      .ok_or_else(bad)?;
    if pool.iter().any(|(seen, _)| *seen == kind) {
      return Err("duplicate hosts in saved pool".to_owned());
    }
    pool.push((kind, cap));
  }
  Ok(pool)
}

/// A host as a saved file names it: one of the four exact names, never an
/// alias or another spelling.
fn saved_host(name: &str) -> Option<HostKind> {
  HOST_KINDS.into_iter().find(|kind| kind.name() == name)
}

/// A saved route's host (or none for no capacity), model and effort.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LaunchRoute {
  /// The host; `None` means no host had capacity.
  pub host: Option<HostKind>,
  /// The model.
  pub model: Option<String>,
  /// The effort.
  pub effort: Option<String>,
}

/// The saved route of one issue, or `None` when it has none.
///
/// # Errors
/// A malformed route.
pub(crate) fn load_launch_route(path: &Path) -> Result<Option<LaunchRoute>, String> {
  let Some(value) = read_strict(path)? else {
    return Ok(None);
  };
  let bad = |why: &str| format!("invalid route {}: {why}", path.display());
  let object = value
    .as_object()
    .ok_or_else(|| bad("expected a JSON object"))?;
  let host = match object.get("host") {
    Some(Value::Null) => None,
    Some(Value::String(name)) => Some(saved_host(name).ok_or_else(|| bad("unknown host"))?),
    _ => return Err(bad("host must be a host name or null")),
  };
  let text = |key: &str| -> Result<Option<String>, String> {
    match object.get(key) {
      None | Some(Value::Null) => Ok(None),
      Some(Value::String(value)) if !value.is_empty() => Ok(Some(value.clone())),
      Some(_) => Err(bad(&format!("{key} must be a non-empty string"))),
    }
  };
  Ok(Some(LaunchRoute {
    host,
    model: text("model")?,
    effort: text("effort")?,
  }))
}

#[cfg(test)]
#[path = "tests/pool_test.rs"]
mod tests;
