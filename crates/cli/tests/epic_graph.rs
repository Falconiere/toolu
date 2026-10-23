//! `toolu epic graph` and `toolu epic route` from the real binary.

#[path = "helpers/epic.rs"]
mod epic;
#[path = "helpers/epic_doc.rs"]
mod epic_doc;
#[path = "helpers/epic_graph.rs"]
mod epic_graph;
#[path = "helpers/epic_out.rs"]
mod epic_out;

use epic::{Home, Res, err};
use epic_doc::doc;
use epic_graph::write_graph;
use epic_out::out;

#[test]
fn a_malformed_epic_is_a_usage_error_and_missing_jira_credentials_a_failure() {
  let home = Home::new().unwrap();
  let bad = home.run(&["epic", "graph", "not a ref"]).unwrap();
  assert_eq!(bad.status.code(), Some(64), "{}", err(&bad));
  assert!(err(&bad).contains("unrecognized epic reference"));
  let jira = home
    .run(&["epic", "graph", "jira:PAY-7", "--repo", "o/r"])
    .unwrap();
  assert_eq!(jira.status.code(), Some(1));
  assert!(err(&jira).contains("JIRA_BASE_URL"));
  assert_eq!(
    home.run(&["epic", "graph"]).unwrap().status.code(),
    Some(64)
  );
}

/// `toolu epic route --graph <graph> <extra…>`.
fn route(home: &Home, graph: &str, extra: &[&str]) -> Res<std::process::Output> {
  let mut args = vec!["epic", "route", "--graph", graph];
  args.extend_from_slice(extra);
  home.run(&args)
}

#[test]
fn route_prints_the_batch_routes_and_a_dry_run_writes_nothing() {
  let home = Home::new().unwrap();
  let graph = write_graph(&home, |_| {}).unwrap();
  let hosts = ["--no-jev", "--dry-run", "--hosts", "claude:2,codex:1"];
  let table = route(&home, &graph, &hosts).unwrap();
  assert_eq!(table.status.code(), Some(0), "{}", err(&table));
  assert!(
    out(&table).contains("Falconiere/comemory#255"),
    "{}",
    out(&table)
  );
  let json = home
    .run(&[
      "--json",
      "epic",
      "route",
      "--graph",
      &graph,
      "--no-jev",
      "--dry-run",
      "--issue",
      "comemory-255",
    ])
    .unwrap();
  let routes = doc(&json).unwrap();
  assert_eq!(routes["routes"][0]["ref"], "Falconiere/comemory#255");
  assert!(!home.path("state/routes").exists());
}

#[test]
fn route_sends_work_to_muse_with_its_tier_model_and_effort() {
  let home = Home::new().unwrap();
  let graph = write_graph(&home, |_| {}).unwrap();
  let json = home
    .run(&[
      "--json",
      "epic",
      "route",
      "--graph",
      &graph,
      "--no-jev",
      "--dry-run",
      "--hosts",
      "muse:1",
      "--issue",
      "comemory-255",
    ])
    .unwrap();
  assert_eq!(json.status.code(), Some(0), "{}", err(&json));
  let routes = doc(&json).unwrap();
  let first = &routes["routes"][0];
  assert_eq!(first["host"], "muse", "{routes}");
  assert!(
    first["model"]
      .as_str()
      .is_some_and(|model| model.starts_with("muse-")),
    "{routes}"
  );
  let unknown = route(&home, &graph, &["--hosts", "nope:1"]).unwrap();
  assert_eq!(unknown.status.code(), Some(64));
  assert!(
    err(&unknown).contains("use one of claude, codex, cursor, muse"),
    "{}",
    err(&unknown)
  );
}

#[test]
fn route_refuses_a_fractional_cap_and_saves_a_route_without_dry_run() {
  let home = Home::new().unwrap();
  let graph = write_graph(&home, |_| {}).unwrap();
  let bad = route(&home, &graph, &["--hosts", "claude:1.5"]).unwrap();
  assert_eq!(bad.status.code(), Some(64));
  let saved = route(&home, &graph, &["--no-jev", "--issue", "comemory-255"]).unwrap();
  assert_eq!(saved.status.code(), Some(0), "{}", err(&saved));
  assert!(home.path("state/routes/comemory-255.json").exists());
}
