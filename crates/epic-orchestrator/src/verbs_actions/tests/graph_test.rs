use toolu_protocol::exit::Exit;
use toolu_runtime::cli::Ctx;
use toolu_runtime::env::Env;

use super::{default_repo, graph, graph_with, route};
use crate::support::matches;

fn saved_graph(dir: &std::path::Path) -> String {
  let mut graph = crate::support::graph_248();
  graph.state_dir = dir.join("state").display().to_string();
  let path = dir.join("graph.json");
  crate::graph::write_graph(&path, &graph).expect("graph");
  path.display().to_string()
}

#[test]
fn a_malformed_epic_is_a_usage_error_and_missing_credentials_a_failure() {
  let env = Env::from_pairs([("HOME", "/tmp")]);
  let bad = graph(
    &matches(&["epic", "graph", "not a ref"]),
    &Ctx::default(),
    &env,
  );
  assert_eq!(bad.exit, Exit::Usage);
  let jira = graph(
    &matches(&["epic", "graph", "jira:PAY-7", "--repo", "o/r"]),
    &Ctx::default(),
    &env,
  );
  assert_eq!(jira.exit, Exit::Failure);
  assert!(
    jira
      .stderr
      .is_some_and(|text| text.contains("JIRA_BASE_URL"))
  );
}

#[test]
fn routes_print_as_a_table_or_a_document_and_a_dry_run_writes_nothing() {
  let tmp = tempfile::tempdir().expect("temp");
  let file = saved_graph(tmp.path());
  let env = Env::from_pairs([
    ("HOME", tmp.path().display().to_string()),
    (
      "TOOLU_RESOURCE_HOME",
      tmp.path().join("r").display().to_string(),
    ),
  ]);
  let line = [
    "epic",
    "route",
    "--graph",
    &file,
    "--no-jev",
    "--dry-run",
    "--hosts",
    "claude:1,codex:1",
  ];
  let table = route(&matches(&line), &Ctx::default(), &env);
  let text = table.stdout.expect("table");
  assert!(
    text.contains("comemory-255") || text.contains("Falconiere/comemory#255"),
    "{text}"
  );
  let json = route(
    &matches(&line),
    &Ctx {
      json: true,
      ..Ctx::default()
    },
    &env,
  );
  let doc: serde_json::Value = serde_json::from_str(&json.stdout.expect("json")).expect("document");
  assert_eq!(doc["routes"].as_array().map(Vec::len), Some(3));
  assert!(!tmp.path().join("state").exists());
  let bad = route(
    &matches(&["epic", "route", "--graph", &file, "--hosts", "claude:0"]),
    &Ctx::default(),
    &env,
  );
  assert_eq!(bad.exit, Exit::Usage);
}

#[test]
fn graph_saves_where_asked_prints_a_table_or_document_and_a_dry_run_writes_nothing() {
  let scenario = crate::scenario_rig::Scenario::base(&[(2, &[]), (3, &[2])]);
  scenario.publish("open");
  let reach = crate::support::reach(&scenario.github, crate::support::FAST);
  let out = scenario.tmp.path().join("out.json");
  let saved = scenario.tmp.path().join("epics/o-r-1/graph.json");
  let run = |line: &[&str], json: bool| {
    let ctx = Ctx {
      json,
      ..Ctx::default()
    };
    graph_with(&matches(line), &ctx, &scenario.env, &reach)
  };
  let out_arg = out.display().to_string();
  let dry = run(
    &[
      "epic",
      "graph",
      "o/r#1",
      "--save",
      "--out",
      &out_arg,
      "--dry-run",
    ],
    false,
  );
  assert_eq!(dry.exit, Exit::Success, "{dry:?}");
  assert!(dry.stdout.unwrap_or_default().contains("o/r#2"));
  assert!(!out.exists() && !saved.exists());
  let table = run(&["epic", "graph", "o/r#1", "--out", &out_arg], false);
  assert_eq!(table.exit, Exit::Success, "{table:?}");
  assert!(out.is_file() && !saved.exists());
  let doc = run(&["epic", "graph", "o/r#1", "--save"], true);
  let doc: serde_json::Value = serde_json::from_str(&doc.stdout.expect("json")).expect("document");
  assert_eq!(doc["issues"].as_array().map(Vec::len), Some(2));
  assert_eq!(
    crate::graph::read_graph(&saved).map(|g| g.issues.len()),
    Ok(2)
  );
}

#[test]
fn a_jira_epic_without_a_github_client_fails_instead_of_skipping_pull_requests() {
  let fixture = toolu_http_test_support::Fixture::start().expect("fixture");
  let mut reach = crate::support::reach(&fixture, crate::support::FAST);
  reach.gh = std::sync::Arc::new(|_env: &Env| Err("GH_TOKEN is not set".to_owned()));
  let env = Env::from_pairs([("HOME", "/tmp")]);
  let line = ["epic", "graph", "jira:PAY-7", "--repo", "o/r"];
  let outcome = graph_with(&matches(&line), &Ctx::default(), &env, &reach);
  assert_eq!(outcome.exit, Exit::Failure);
  assert!(
    outcome
      .stderr
      .is_some_and(|text| text.contains("GH_TOKEN is not set"))
  );
}

#[test]
fn the_default_repository_is_the_flag_none_for_github_else_the_current_one() {
  use crate::tracker::TrackerKind::{Github, Jira, Linear};
  let unused = || -> Result<String, String> { Err("asked".to_owned()) };
  let flagged = matches(&["epic", "graph", "jira:PAY-7", "--repo", "o/r"]);
  assert_eq!(default_repo(&flagged, Jira, unused), Ok("o/r".to_owned()));
  let bare = matches(&["epic", "graph", "o/r#1"]);
  assert_eq!(default_repo(&bare, Github, unused), Ok(String::new()));
  let here = || Ok("acme/payments".to_owned());
  assert_eq!(
    default_repo(&bare, Linear, here),
    Ok("acme/payments".to_owned())
  );
  assert_eq!(default_repo(&bare, Jira, unused), Err("asked".to_owned()));
}
