//! Allow-listed machine, host and job resource snapshots.

use std::time::{Duration, UNIX_EPOCH};

use serde_json::{Value, json};
use toolu_engine::resources::pressure::pressure_of;
use toolu_engine::resources::store::{
  ResourcePolicy, leases, read_resource_state, resource_policy,
};
use toolu_runtime::json::ordered::Ordered;
use toolu_state::time::{epoch_millis, iso_millis, parse_iso};

use crate::hosts::HOST_KINDS;
use crate::paths::Paths;
use crate::waiting_jobs;

/// Resource documents at one observation time.
pub(crate) struct Snapshot {
  now_ms: u64,
  policy: ResourcePolicy,
  state: Ordered,
  waiting: Vec<Value>,
}

impl Snapshot {
  /// Read current resource state and policy.
  ///
  /// # Errors
  /// A resource file is invalid or unreadable.
  pub(crate) fn open(paths: &Paths, now_ms: u64) -> Result<Self, String> {
    Ok(Self {
      now_ms,
      policy: resource_policy(&paths.root)?,
      state: read_resource_state(&paths.root)?,
      waiting: waiting_jobs::list(&paths.root, now_ms)?,
    })
  }

  /// Per-host caps, running counts and cooldowns.
  pub(crate) fn hosts(&self) -> Value {
    let items: Vec<Value> = HOST_KINDS
      .iter()
      .map(|kind| self.host(kind.name()))
      .collect();
    json!({"schema": 1, "items": items})
  }

  /// Machine pressure, policy and leases.
  pub(crate) fn machine(&self) -> Value {
    let pressure = self.state.get("pressure").and_then(pressure_of);
    let sample = pressure.as_ref().map(|pressure| &pressure.sample);
    let agent_leases: Vec<Value> = leases(&self.state)
      .iter()
      .filter(|lease| text(lease, "type") == Some("agent"))
      .map(|lease| public_lease(lease, self.now_ms))
      .collect();
    let job_leases: Vec<Value> = leases(&self.state)
      .iter()
      .filter(|lease| text(lease, "type") == Some("job"))
      .map(|lease| public_lease(lease, self.now_ms))
      .collect();
    json!({
      "schema": 1,
      "load": sample.map(|sample| sample.load),
      "available_memory_bytes": sample.map(|sample| sample.available_bytes),
      "steal": sample.and_then(|sample| sample.steal),
      "pressure": pressure.as_ref().map(|pressure| json!({
        "held": pressure.held, "reason": pressure.reason,
      })),
      "policy": {"max_agents": self.policy.max_agents, "max_jobs": self.policy.max_jobs,
        "pressure": self.policy.pressure},
      "agent_leases": agent_leases,
      "job_leases": job_leases,
    })
  }

  /// Running and waiting expensive jobs.
  pub(crate) fn jobs(&self) -> Value {
    let running: Vec<Value> = leases(&self.state)
      .iter()
      .filter(|lease| text(lease, "type") == Some("job"))
      .map(|lease| public_lease(lease, self.now_ms))
      .collect();
    json!({"schema": 1, "running": running, "waiting": self.waiting})
  }

  fn host(&self, host: &str) -> Value {
    let cap = self
      .policy
      .hosts
      .iter()
      .find(|(name, _)| name == host)
      .map_or(self.policy.max_agents, |(_, cap)| *cap);
    let running = leases(&self.state)
      .iter()
      .filter(|lease| {
        text(lease, "type") == Some("agent")
          && (text(lease, "host") == Some(host) || text(lease, "pendingHost") == Some(host))
      })
      .count();
    let cooldown = self
      .state
      .get("cooldowns")
      .and_then(|cooldowns| cooldowns.get(host));
    let until = cooldown.and_then(|cooldown| integer(cooldown, "until"));
    let active = until.filter(|until| *until > self.now_ms);
    let paused = crate::hosts::parse_host_kind(host)
      .and_then(|kind| crate::host_pause::paused_hosts(&self.state).get(&kind).cloned());
    json!({
      "host": host,
      "cap": cap,
      "running": running,
      "cooldown_until": active.map(|until| iso_millis(UNIX_EPOCH + Duration::from_millis(until))),
      "reason": active.and_then(|_| cooldown.and_then(|cooldown| text(cooldown, "reason"))),
      "paused": paused.is_some(),
      "paused_by": paused.as_ref().map(|paused| paused.by.clone()),
      "paused_at": paused.map(|paused| iso_millis(UNIX_EPOCH + Duration::from_millis(paused.at))),
    })
  }
}

fn text<'a>(value: &'a Ordered, key: &str) -> Option<&'a str> {
  match value.get(key) {
    Some(Ordered::String(text)) => Some(text),
    _ => None,
  }
}

fn integer(value: &Ordered, key: &str) -> Option<u64> {
  match value.get(key) {
    Some(Ordered::Number(number)) => number.as_u64(),
    _ => None,
  }
}

fn public_lease(lease: &Ordered, now_ms: u64) -> Value {
  let age_ms = text(lease, "createdAt")
    .and_then(parse_iso)
    .map(epoch_millis)
    .map(|created| now_ms.saturating_sub(created));
  json!({
    "owner": text(lease, "key"),
    "worktree": text(lease, "worktree"),
    "host": text(lease, "host"),
    "stage": text(lease, "stage"),
    "age_ms": age_ms,
  })
}

#[cfg(test)]
#[path = "tests/status_resources_test.rs"]
mod tests;
