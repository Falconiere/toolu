use std::path::Path;

use serde_json::{Value, json};
use toolu_engine::resources::leases::{LeaseKind, LeaseRequest, acquire_lease};
use toolu_engine::resources::transitions::{MigrationCaps, prepare_agent_migration};

use crate::graph::Graph;
use crate::route::table::default_table;
use crate::route::{RouteOptions, parse_hosts, route_issues};
use crate::support::graph_248;

fn graph_in(root: &Path) -> Graph {
  let mut graph = graph_248();
  graph.state_dir = root.join("state").display().to_string();
  graph
}

fn options(table: &crate::route::RoutingTable) -> RouteOptions<'_> {
  RouteOptions {
    jev: None,
    reroute: false,
    table,
    resource_root: None,
    now_ms: 1_700_000_000_000,
    persist: true,
  }
}

fn hosts(
  graph: &Graph,
  wanted: &[&str],
  spec: &str,
  cap: u64,
  options: &RouteOptions<'_>,
) -> Vec<Option<String>> {
  let wanted: Vec<String> = wanted.iter().map(|name| (*name).to_owned()).collect();
  let pool = parse_hosts(spec, cap).expect("pool");
  let (routes, _) = route_issues(graph, &wanted, &pool, options).expect("routes");
  routes.into_iter().map(|route| route.host).collect()
}

const PAIR: [&str; 2] = ["Falconiere/comemory#255", "CodaSignal/comemory.io#183"];

#[test]
fn a_batch_spreads_across_hosts_and_each_route_persists() {
  let tmp = tempfile::tempdir().expect("temp");
  let graph = graph_in(tmp.path());
  let table = default_table();
  assert_eq!(
    hosts(&graph, &PAIR, "claude:1,codex:1", 1, &options(&table)),
    [Some("claude".into()), Some("codex".into())]
  );
  let saved: Value = serde_json::from_str(
    &std::fs::read_to_string(Path::new(&graph.state_dir).join("routes/comemory-255.json"))
      .expect("route"),
  )
  .expect("json");
  assert_eq!(saved["host"], "claude");
  assert_eq!(saved["source"], "heuristic");
  let pool: Value = serde_json::from_str(
    &std::fs::read_to_string(Path::new(&graph.state_dir).join("pool.json")).expect("pool"),
  )
  .expect("json");
  assert_eq!(
    pool,
    json!([{"kind": "claude", "cap": 1}, {"kind": "codex", "cap": 1}])
  );
}

#[test]
fn a_relaunch_keeps_its_route_and_a_cooling_host_forces_a_move() {
  let tmp = tempfile::tempdir().expect("temp");
  let graph = graph_in(tmp.path());
  let table = default_table();
  let opts = options(&table);
  let pool = parse_hosts("claude:2,codex:2", 2).expect("pool");
  let wanted = vec![PAIR[0].to_owned()];
  let first = route_issues(&graph, &wanted, &pool, &opts)
    .expect("first")
    .0
    .remove(0);
  let later = RouteOptions {
    now_ms: opts.now_ms + 60_000,
    ..options(&table)
  };
  let again = route_issues(&graph, &wanted, &pool, &later)
    .expect("again")
    .0
    .remove(0);
  assert_eq!(
    (&again.host, &again.decided_at),
    (&first.host, &first.decided_at)
  );
  let until = "2999-01-01T00:00:00Z";
  crate::disk::write_value(
    &Path::new(&graph.state_dir).join("hosts.json"),
    &json!({first.host.clone().expect("host"): {"until": until}}),
  )
  .expect("hosts");
  let moved = route_issues(&graph, &wanted, &pool, &later)
    .expect("moved")
    .0
    .remove(0);
  assert_ne!(moved.host, first.host);
  assert_eq!(moved.tier, first.tier);
}

#[test]
fn no_capacity_leaves_the_host_empty_and_cached_routes_consume_slots() {
  let tmp = tempfile::tempdir().expect("temp");
  let graph = graph_in(tmp.path());
  let table = default_table();
  let opts = options(&table);
  assert_eq!(
    hosts(&graph, &PAIR, "claude:1", 1, &opts),
    [Some("claude".into()), None]
  );
  assert_eq!(
    hosts(&graph, &PAIR, "claude:1", 1, &opts),
    [Some("claude".into()), None]
  );
  assert_eq!(
    hosts(&graph, &PAIR, "codex:1", 1, &opts),
    [Some("codex".into()), None]
  );
}

#[test]
fn routing_counts_both_source_and_reserved_destination_across_epics() {
  let tmp = tempfile::tempdir().expect("temp");
  let graph = graph_in(tmp.path());
  let root = tmp.path().join("resources");
  let request = LeaseRequest {
    kind: LeaseKind::Agent,
    key: "other-1".into(),
    state_dir: "other-epic".into(),
    host: Some("claude".into()),
    host_cap: None,
    epic_cap: None,
    worktree: None,
  };
  let lease = acquire_lease(&root, &request, std::time::SystemTime::now()).expect("lease");
  let token = match lease.get("token") {
    Some(toolu_runtime::json::ordered::Ordered::String(t)) => t.clone(),
    _ => panic!("token"),
  };
  let caps = MigrationCaps {
    host_cap: 1,
    epic_cap: None,
  };
  prepare_agent_migration(&root, &token, "codex", caps, std::time::SystemTime::now())
    .expect("reserve");
  let table = default_table();
  let opts = RouteOptions {
    resource_root: Some(root),
    ..options(&table)
  };
  assert_eq!(
    hosts(&graph, &["comemory-255"], "claude:1,codex:1", 2, &opts),
    [None]
  );
}

#[test]
fn issues_resolve_by_ref_or_key_and_unknown_ones_are_refused() {
  let tmp = tempfile::tempdir().expect("temp");
  let graph = graph_in(tmp.path());
  let table = default_table();
  let pool = parse_hosts("claude:3", 3).expect("pool");
  let wanted = vec![
    "comemory-255".to_owned(),
    PAIR[1].to_owned(),
    "comemory-255".to_owned(),
  ];
  let (routes, _) = route_issues(&graph, &wanted, &pool, &options(&table)).expect("routes");
  let refs: Vec<&str> = routes
    .iter()
    .map(|route| route.reference.as_str())
    .collect();
  assert_eq!(refs, PAIR);
  let err =
    route_issues(&graph, &["nope-1".to_owned()], &pool, &options(&table)).expect_err("unknown");
  assert_eq!(err, "nope-1 is not in the graph");
  let dry = RouteOptions {
    persist: false,
    ..options(&table)
  };
  let fresh = graph_in(&tmp.path().join("dry"));
  route_issues(&fresh, &[PAIR[0].to_owned()], &pool, &dry).expect("dry");
  assert!(!Path::new(&fresh.state_dir).exists());
}

#[test]
fn an_unavailable_jev_falls_back_to_heuristic_tiers_with_a_note() {
  let tmp = tempfile::tempdir().expect("temp");
  let graph = graph_in(tmp.path());
  let table = default_table();
  let opts = RouteOptions {
    jev: Some(Err("TYPESAFE_API_KEY not set".into())),
    ..options(&table)
  };
  let pool = parse_hosts("claude:1", 1).expect("pool");
  let (routes, note) = route_issues(&graph, &[PAIR[0].to_owned()], &pool, &opts).expect("routes");
  assert_eq!(
    note.as_deref(),
    Some("jev unavailable (TYPESAFE_API_KEY not set); heuristic tiers used")
  );
  assert_eq!(routes[0].source, "heuristic");
}
