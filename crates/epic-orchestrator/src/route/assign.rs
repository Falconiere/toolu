//! Assign each wanted issue a tier, host, model and effort (`routeIssues`).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use toolu_jev_client::Jev;

use super::capacity::Capacity;
use super::pool::{HostPool, pick_host};
use super::score::{Score, jev_scores};
use super::table::{RoutingTable, TIERS};
use super::tier::{Evidence, heuristic_tier};
use crate::graph::{Graph, GraphIssue};
use crate::hosts::{HostKind, parse_host_kind};

/// One persisted route (`routes/<key>.json`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct Route {
  /// The herdr key.
  pub key: String,
  /// The tracker reference.
  #[serde(rename = "ref")]
  pub reference: String,
  /// Tier name.
  pub tier: String,
  /// Jev's expected score.
  pub score: Option<f64>,
  /// Jev's confidence.
  pub confidence: Option<f64>,
  /// `jev` or `heuristic`.
  pub source: String,
  /// The host, or `None` when none had capacity.
  pub host: Option<String>,
  /// The model.
  pub model: Option<String>,
  /// The effort.
  pub effort: Option<String>,
  /// When the route was decided (ISO seconds).
  pub decided_at: String,
}

/// How one routing pass runs.
pub(crate) struct RouteOptions<'a> {
  /// `None` skips Jev; `Some(Err)` says why it is unavailable.
  pub jev: Option<Result<&'a Jev, String>>,
  /// Choose a host again even when the saved one is still valid.
  pub reroute: bool,
  /// The routing table.
  pub table: &'a RoutingTable,
  /// The machine resource root whose leases and cooldowns count.
  pub resource_root: Option<PathBuf>,
  /// The clock, milliseconds.
  pub now_ms: u64,
  /// Write routes and `pool.json`; a dry run writes nothing.
  pub persist: bool,
}

fn targets<'g>(graph: &'g Graph, wanted: &[String]) -> Result<Vec<&'g GraphIssue>, String> {
  let mut seen = BTreeSet::new();
  let mut out = Vec::new();
  for name in wanted {
    let issue = graph
      .issues
      .iter()
      .find(|issue| issue.reference == *name)
      .or_else(|| graph.issues.iter().find(|issue| issue.key == *name))
      .ok_or_else(|| format!("{name} is not in the graph"))?;
    if seen.insert(issue.key.clone()) {
      out.push(issue);
    }
  }
  Ok(out)
}

fn evidence(issue: &GraphIssue) -> Evidence {
  Evidence {
    key: issue.key.clone(),
    title: issue.title.clone(),
    labels: issue.labels.clone(),
    excerpt: issue.excerpt.clone(),
    open_blockers: issue.open_blockers.len(),
    unblocks: issue.unblocks,
  }
}

/// Route `wanted` (references or keys) over `pool`.
///
/// # Errors
/// An unknown issue, unreadable resource state, or a route that cannot be written.
pub(crate) fn route_issues(
  graph: &Graph,
  wanted: &[String],
  pool: &HostPool,
  options: &RouteOptions<'_>,
) -> Result<(Vec<Route>, Option<String>), String> {
  let state_dir = PathBuf::from(&graph.state_dir);
  let chosen = targets(graph, wanted)?;
  let existing: BTreeMap<String, Route> = chosen
    .iter()
    .filter_map(|issue| saved_route(&state_dir, &issue.key).map(|route| (issue.key.clone(), route)))
    .collect();
  let unscored: Vec<Evidence> = chosen
    .iter()
    .filter(|issue| !existing.contains_key(&issue.key))
    .map(|issue| evidence(issue))
    .collect();
  let (scores, note) = score_all(options, &unscored);
  let capacity = Capacity::load(&state_dir, options.resource_root.as_deref(), options.now_ms)?;
  if options.persist {
    let rows: Vec<Value> = pool
      .iter()
      .map(|(kind, cap)| json!({"kind": kind.name(), "cap": cap}))
      .collect();
    crate::disk::write_value(&state_dir.join("pool.json"), &Value::Array(rows))?;
  }
  let mut routes = Vec::new();
  let mut pass = Pass {
    pool,
    options,
    capacity,
  };
  for issue in chosen {
    let route = pass.route(issue, existing.get(&issue.key), scores.get(&issue.key));
    if options.persist {
      let path = state_dir.join("routes").join(format!("{}.json", issue.key));
      crate::disk::write_value(
        &path,
        &serde_json::to_value(&route).map_err(|err| err.to_string())?,
      )?;
    }
    routes.push(route);
  }
  Ok((routes, note))
}

fn saved_route(state_dir: &Path, key: &str) -> Option<Route> {
  let value =
    crate::disk::read_value(&state_dir.join("routes").join(format!("{key}.json"))).ok()?;
  serde_json::from_value(value).ok()
}

fn score_all(
  options: &RouteOptions<'_>,
  unscored: &[Evidence],
) -> (BTreeMap<String, Score>, Option<String>) {
  if unscored.is_empty() {
    return (BTreeMap::new(), None);
  }
  let outcome = match &options.jev {
    None => return (BTreeMap::new(), None),
    Some(Err(reason)) => Err(reason.clone()),
    Some(Ok(jev)) => jev_scores(jev, unscored),
  };
  match outcome {
    Ok(scores) => (scores, None),
    Err(reason) => (
      BTreeMap::new(),
      Some(format!("jev unavailable ({reason}); heuristic tiers used")),
    ),
  }
}

struct Pass<'a> {
  pool: &'a HostPool,
  options: &'a RouteOptions<'a>,
  capacity: Capacity,
}

/// The tier and its evidence: the saved route's, else Jev's, else the heuristic's.
fn scored(issue: &GraphIssue, previous: Option<&Route>, score: Option<&Score>) -> Score {
  match (previous, score) {
    (Some(prev), _) => Score {
      tier: TIERS
        .iter()
        .position(|name| *name == prev.tier)
        .unwrap_or(1),
      expected: prev.score,
      confidence: prev.confidence,
      source: prev.source.clone(),
    },
    (None, Some(found)) => found.clone(),
    (None, None) => Score {
      tier: heuristic_tier(&evidence(issue)),
      expected: None,
      confidence: None,
      source: "heuristic".to_owned(),
    },
  }
}

impl Pass<'_> {
  fn route(
    &mut self,
    issue: &GraphIssue,
    previous: Option<&Route>,
    score: Option<&Score>,
  ) -> Route {
    let live = self.capacity.active.get(&issue.key).copied();
    let kept = previous.and_then(|prev| self.keep(prev, live));
    let scored = scored(issue, previous, score);
    let tier_name = TIERS.get(scored.tier).copied().unwrap_or("standard");
    let table = self.options.table;
    let prefer = table.prefer.get(tier_name).cloned().unwrap_or_default();
    let blocked: BTreeSet<HostKind> = self
      .capacity
      .cooling
      .union(&self.capacity.paused)
      .copied()
      .collect();
    let host = kept.or_else(|| pick_host(self.pool, &self.capacity.used, &blocked, &prefer));
    if let Some(host) = host.filter(|host| live != Some(*host)) {
      *self.capacity.used.entry(host).or_default() += 1;
    }
    let choice = host
      .and_then(|host| table.hosts.get(&host)?.get(scored.tier).cloned())
      .unwrap_or_default();
    let decided_at = match (previous, kept) {
      (Some(prev), Some(_)) => prev.decided_at.clone(),
      _ => toolu_state::time::iso_seconds(
        std::time::UNIX_EPOCH + std::time::Duration::from_millis(self.options.now_ms),
      ),
    };
    Route {
      key: issue.key.clone(),
      reference: issue.reference.clone(),
      tier: tier_name.to_owned(),
      score: scored.expected,
      confidence: scored.confidence,
      source: scored.source,
      host: host.map(|host| host.name().to_owned()),
      model: choice.model,
      effort: choice.effort,
      decided_at,
    }
  }

  /// The saved host when it is still allowed: not rerouting, not cooling, in
  /// the pool, and with room (or already the live host).
  fn keep(&self, previous: &Route, live: Option<HostKind>) -> Option<HostKind> {
    let host = previous.host.as_deref().and_then(parse_host_kind)?;
    let used = &self.capacity.used;
    let room = self.pool.iter().any(|(kind, cap)| {
      *kind == host && (used.get(kind).copied().unwrap_or(0) < *cap || live == Some(host))
    });
    (!self.options.reroute
      && !self.capacity.cooling.contains(&host)
      && !self.capacity.paused.contains(&host)
      && room)
      .then_some(host)
  }
}

#[cfg(test)]
#[path = "tests/assign_test.rs"]
mod tests;
