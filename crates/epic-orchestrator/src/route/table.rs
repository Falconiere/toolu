//! The routing table: per host, one model and effort per tier, and an optional
//! per-tier host preference. `routing.json` under the epic home (or
//! `EPIC_ROUTING_FILE`) overrides the defaults host by host.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use toolu_runtime::env::Env;

use crate::hosts::{HostKind, parse_host_kind};
use crate::refs::epics_home;

/// The four tiers, lowest first.
pub(crate) const TIERS: [&str; 4] = ["trivial", "standard", "complex", "critical"];

/// One tier's model and effort on one host; unset keeps the host's default.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ModelChoice {
  /// The model.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub model: Option<String>,
  /// The effort.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub effort: Option<String>,
}

/// Choices per host and tier, and preferred hosts per tier.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct RoutingTable {
  /// Four choices per host.
  pub hosts: BTreeMap<HostKind, Vec<ModelChoice>>,
  /// Preferred hosts by tier name.
  pub prefer: BTreeMap<String, Vec<HostKind>>,
}

fn choice(model: Option<&str>, effort: Option<&str>) -> ModelChoice {
  ModelChoice {
    model: model.map(str::to_owned),
    effort: effort.map(str::to_owned),
  }
}

/// Effort rises with the tier and the model steps up where a host has a
/// stronger one. `OpenCode` keeps its configured model.
pub(crate) fn default_table() -> RoutingTable {
  let hosts = [
    (
      HostKind::Claude,
      vec![
        choice(Some("sonnet"), Some("low")),
        choice(Some("sonnet"), Some("medium")),
        choice(Some("opus"), Some("high")),
        choice(Some("opus"), Some("xhigh")),
      ],
    ),
    (
      HostKind::Codex,
      ["low", "medium", "high", "xhigh"]
        .iter()
        .map(|effort| choice(Some("gpt-6-sol"), Some(effort)))
        .collect(),
    ),
    (
      HostKind::Cursor,
      vec![
        choice(Some("composer-2.5"), None),
        choice(Some("gpt-5.6-sol-high"), None),
        choice(Some("claude-opus-5-thinking-high"), None),
        choice(Some("gpt-5.6-sol-xhigh"), None),
      ],
    ),
    (HostKind::Opencode, vec![ModelChoice::default(); 4]),
  ];
  RoutingTable {
    hosts: hosts.into_iter().collect(),
    prefer: BTreeMap::new(),
  }
}

/// The table: defaults, with each host and preference `routing.json` names replaced.
pub(crate) fn load_table(env: &Env) -> RoutingTable {
  let path = env.get("EPIC_ROUTING_FILE").map_or_else(
    || epics_home(env).join("routing.json"),
    std::path::PathBuf::from,
  );
  let user = crate::disk::read_value(&path).unwrap_or(Value::Null);
  let mut table = default_table();
  if let Some(hosts) = user.get("hosts").and_then(Value::as_object) {
    for (name, rows) in hosts {
      if let (Some(kind), Ok(rows)) = (parse_host_kind(name), serde_json::from_value(rows.clone()))
      {
        table.hosts.insert(kind, rows);
      }
    }
  }
  if let Some(prefer) = user.get("prefer").and_then(Value::as_object) {
    for (tier, names) in prefer {
      let kinds = names
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .filter_map(parse_host_kind)
        .collect();
      table.prefer.insert(tier.clone(), kinds);
    }
  }
  table
}

#[cfg(test)]
#[path = "tests/table_test.rs"]
mod tests;
