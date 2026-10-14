use std::collections::{BTreeMap, BTreeSet};

use serde_json::json;

use super::{LaunchRoute, cooling_hosts, pick_host};
use crate::hosts::HostKind::{Claude, Codex, Cursor};
use crate::model::Issue;
use crate::paths::Paths;
use crate::route::{HostPool, load_host_pool, load_launch_route, parse_hosts};
use crate::server::{Engine, Fault};

#[test]
fn host_pools_parse_caps_and_refuse_bad_ones() {
  assert_eq!(
    parse_hosts("claude:2, codex, cursor-agent:1", 3),
    Ok(vec![(Claude, 2), (Codex, 3), (Cursor, 1)])
  );
  assert_eq!(parse_hosts("claude:", 3), Ok(vec![(Claude, 3)]));
  for spec in [
    "claude:0",
    "claude:-1",
    "claude:1.5",
    "claude:NaN",
    "claude:Infinity",
    "claude:1,claude:2",
    "claude:1:2",
  ] {
    assert!(parse_hosts(spec, 3).is_err(), "{spec}");
  }
  assert_eq!(
    parse_hosts("claude:0", 3),
    Err("invalid host capacity: claude:0".to_owned())
  );
  assert_eq!(parse_hosts(" , ", 3), Err("host pool is empty".to_owned()));
  assert!(parse_hosts("gemini", 3).is_err());
}

#[test]
fn the_most_free_host_wins_and_cooling_or_full_hosts_are_skipped() {
  let pool: HostPool = parse_hosts("claude:2,codex:2", 2).expect("pool");
  let none = BTreeSet::new();
  assert_eq!(pick_host(&pool, &BTreeMap::new(), &none, &[]), Some(Claude));
  assert_eq!(
    pick_host(&pool, &BTreeMap::from([(Claude, 1)]), &none, &[]),
    Some(Codex)
  );
  assert_eq!(
    pick_host(&pool, &BTreeMap::new(), &BTreeSet::from([Claude]), &[]),
    Some(Codex)
  );
  assert_eq!(
    pick_host(
      &pool,
      &BTreeMap::from([(Claude, 2), (Codex, 2)]),
      &none,
      &[]
    ),
    None
  );
  assert_eq!(
    pick_host(&pool, &BTreeMap::new(), &none, &[Codex]),
    Some(Codex)
  );
}

#[test]
fn cooldowns_expire() {
  let now = toolu_state::time::epoch_millis(
    toolu_state::time::parse_iso("2026-09-26T12:00:00Z").expect("now"),
  );
  let cooldowns = json!({
    "codex": {"until": "2026-09-26T13:00:00Z"},
    "claude": {"until": "2026-09-26T11:00:00Z"},
    "cursor": {"until": now + 1}
  });
  assert_eq!(
    cooling_hosts(&cooldowns, now),
    BTreeSet::from([Codex, Cursor])
  );
}

#[test]
fn saved_pools_load_strictly() {
  let tmp = tempfile::tempdir().expect("temp");
  let pool = tmp.path().join("pool.json");
  assert_eq!(
    load_host_pool(&pool, vec![(Claude, 3)]),
    Ok(vec![(Claude, 3)])
  );
  std::fs::write(&pool, r#"[{"kind":"codex","cap":2}]"#).expect("pool");
  assert_eq!(load_host_pool(&pool, vec![]), Ok(vec![(Codex, 2)]));
  for bad in [
    "[]",
    "{",
    r#"[{"kind":"codex","cap":0}]"#,
    r#"[{"kind":"x","cap":1}]"#,
    r#"[{"kind":" Claude ","cap":1}]"#,
    r#"[{"kind":"claude-code","cap":1}]"#,
  ] {
    std::fs::write(&pool, bad).expect("pool");
    assert!(load_host_pool(&pool, vec![]).is_err(), "{bad}");
  }
  std::fs::write(
    &pool,
    r#"[{"kind":"codex","cap":1},{"kind":"codex","cap":2}]"#,
  )
  .expect("pool");
  assert_eq!(
    load_host_pool(&pool, vec![]),
    Err("duplicate hosts in saved pool".to_owned())
  );
}

#[test]
fn saved_launch_routes_load_strictly() {
  let tmp = tempfile::tempdir().expect("temp");
  let route = tmp.path().join("route.json");
  assert_eq!(load_launch_route(&route), Ok(None));
  std::fs::write(&route, r#"{"host":null}"#).expect("route");
  assert_eq!(
    load_launch_route(&route),
    Ok(Some(LaunchRoute {
      host: None,
      model: None,
      effort: None
    }))
  );
  std::fs::write(&route, r#"{"host":"codex","model":"m","effort":"high"}"#).expect("route");
  assert_eq!(
    load_launch_route(&route)
      .expect("route")
      .and_then(|r| r.host),
    Some(Codex)
  );
  for bad in [
    "{",
    "[]",
    "null",
    r#"{"host":"gemini"}"#,
    r#"{"host":"Codex"}"#,
    r#"{"host":"cursor-agent"}"#,
    r#"{"host":"codex","model":""}"#,
    r#"{"model":"m"}"#,
  ] {
    std::fs::write(&route, bad).expect("route");
    assert!(load_launch_route(&route).is_err(), "{bad}");
  }
}

#[test]
fn a_cooldown_written_by_limit_moves_the_route() {
  let tmp = tempfile::tempdir().expect("temp");
  let mut graph = crate::support::graph_248();
  graph.state_dir = tmp.path().join("state").display().to_string();
  let engine_root = tmp.path().join("engine");
  let now_ms = 1_800_000_000_000;
  let table = crate::route::table::default_table();
  let options = crate::route::RouteOptions {
    jev: None,
    reroute: false,
    table: &table,
    resource_root: Some(engine_root.clone()),
    now_ms,
    persist: true,
  };
  let pool = parse_hosts("claude:2,codex:2", 2).expect("pool");
  let wanted = vec!["Falconiere/comemory#255".to_owned()];
  let host = |graph: &crate::graph::Graph| {
    let (routes, _) = crate::route::route_issues(graph, &wanted, &pool, &options).expect("route");
    routes.into_iter().next().and_then(|route| route.host)
  };
  assert_eq!(host(&graph).as_deref(), Some("claude"));
  let mut engine = Engine::open(Paths::at(&engine_root), None, Fault::None).expect("open");
  engine.world.now_ms = now_ms;
  let mut issue = Issue::blank("comemory-255", "comemory-248", &graph.state_dir, now_ms);
  issue.kind = "claude".to_owned();
  engine.world.issues.insert("comemory-255".to_owned(), issue);
  crate::limit::on_failed(
    &mut engine,
    "running",
    "rate-limited: usage limit",
    "comemory-255",
  )
  .expect("cool");
  assert_eq!(host(&graph).as_deref(), Some("codex"));
  std::fs::remove_file(std::path::Path::new(&graph.state_dir).join("hosts.json")).expect("local");
  assert_eq!(
    host(&graph).as_deref(),
    Some("codex"),
    "the shared cooldown alone still cools"
  );
}
