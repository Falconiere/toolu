use std::sync::mpsc;

use serde_json::{Value, json};
use toolu_http_test_support::Reply;

use crate::scenario_rig::{Scenario, key};
use crate::server::{Engine, Stop};

fn stage(engine: &Engine, key: &str) -> String {
  engine
    .world
    .issues
    .get(key)
    .map(|issue| issue.stage.clone())
    .unwrap_or_default()
}

fn release(engine: &mut Engine, key: &str) -> Value {
  let (tx, rx) = mpsc::channel();
  let request = json!({"op": "release", "key": key});
  crate::dispatch_ops::reply_release(engine, &request, &tx).expect("release");
  rx.recv().expect("reply")
}

#[test]
fn scenario_coupling_holds_the_overlapping_member_until_its_release() {
  let migration = "Adds migration 0042_add_users to the schema.";
  let bodies = [
    (2, migration),
    (3, migration),
    (4, "Touches the docs index."),
  ];
  let scenario = Scenario::with_bodies(&[(2, &[]), (3, &[]), (4, &[])], 3, &bodies);
  let mut engine = scenario.engine();
  assert_eq!(engine.pump().expect("pump"), Stop::Idle);
  assert_eq!(stage(&engine, &key(2)), "running");
  assert_eq!(
    stage(&engine, &key(3)),
    "",
    "the lower-priority overlap is held"
  );
  assert_eq!(
    stage(&engine, &key(4)),
    "running",
    "an unrelated member launches"
  );
  let held = &engine.world.attention[0];
  assert_eq!(
    (held.kind.as_str(), held.key.as_str()),
    ("coupling-hold", key(3).as_str())
  );
  assert!(held.note.contains(&key(2)), "{}", held.note);
  assert!(
    scenario
      .journal()
      .iter()
      .any(|row| row.name == "hold" && row.key == key(3))
  );
  assert_release(&scenario, &mut engine);
}

/// `release` refuses an unknown and an unheld key, then launches the held one.
fn assert_release(scenario: &Scenario, engine: &mut Engine) {
  assert_eq!(
    release(engine, "o-r-9")["error"],
    "no registered issue o-r-9"
  );
  assert_eq!(
    release(engine, &key(4))["error"],
    format!("{} is not held", key(4))
  );
  assert_eq!(release(engine, &key(3))["released"], true);
  assert_eq!(
    stage(engine, &key(3)),
    "running",
    "a released issue launches unchecked"
  );
  assert!(
    scenario
      .journal()
      .iter()
      .any(|row| row.name == "release" && row.key == key(3))
  );
}

fn jira_issue(key: &str, blocked_by: Option<&str>) -> Value {
  let links: Vec<Value> = blocked_by
    .map(|blocker| {
      json!({"type": {"inward": "is blocked by", "outward": "blocks"},
      "inwardIssue": {"key": blocker, "fields": {"status": {"statusCategory": {"key": "new"}}}}})
    })
    .into_iter()
    .collect();
  json!({"key": key, "fields": {
    "summary": format!("{key} work"),
    "status": {"statusCategory": {"key": "new"}},
    "description": {"type": "doc", "content": [
      {"type": "paragraph", "content": [{"type": "text", "text": "Repo: o/r"}]}
    ]},
    "labels": [], "issuelinks": links,
  }})
}

#[test]
fn scenario_jira_launches_only_the_unblocked_child() {
  let scenario = Scenario::base(&[]);
  let routes = [
    (
      "/rest/api/3/issue/PAY-7?fields=summary,status",
      jira_issue("PAY-7", None),
    ),
    (
      "/rest/api/3/search/jql",
      json!({"issues": [jira_issue("PAY-12", None), jira_issue("PAY-13", Some("PAY-12"))]}),
    ),
    ("/repos/o/r", json!({"default_branch": "main"})),
  ];
  for (path, body) in routes {
    scenario
      .github
      .route(path, Reply::new(200, body.to_string()))
      .expect("route");
  }
  scenario.graph(("PAY-7", "jira", "o/r"), 2);
  let mut engine = scenario.engine();
  assert_eq!(engine.pump().expect("pump"), Stop::Idle);
  let stages: Vec<(String, String)> = engine
    .world
    .issues
    .values()
    .map(|issue| (issue.key.clone(), issue.stage.clone()))
    .collect();
  assert_eq!(
    stages,
    [
      ("pay-12".to_owned(), "running".to_owned()),
      ("pay-13".to_owned(), String::new())
    ]
  );
}

#[test]
fn scenario_opencode_failure_and_disappearance_reach_the_orchestrator() {
  let scenario = Scenario::new(&[(2, &[])], 1);
  scenario.pool(&["opencode"]);
  let mut engine = scenario.engine();
  assert_eq!(engine.pump().expect("pump"), Stop::Idle);
  assert_eq!(
    scenario.record(2)["kind"],
    "opencode",
    "{:?}",
    engine.world.attention
  );
  scenario.report_note(&mut engine, 2, "failed", "tests fail on main");
  let failed = &engine.world.attention[0];
  assert_eq!(
    (failed.kind.as_str(), failed.note.as_str()),
    ("failed", "tests fail on main")
  );
  scenario.lose_agent(2);
  crate::schedule::note_event(&mut engine.world, &key(2), "gone");
  assert_eq!(engine.pump().expect("pump"), Stop::Idle);
  assert_eq!(engine.world.issues[&key(2)].launches, 1);
  assert_eq!(
    stage(&engine, &key(2)),
    "running",
    "{:?}",
    engine.world.attention
  );
  assert_eq!(scenario.record(2)["kind"], "opencode");
}

#[test]
fn scenario_a_safe_epic_launches_its_workers_with_approval_prompts_on() {
  for safe in [true, false] {
    let scenario = Scenario::new(&[(2, &[])], 1);
    let settings = crate::epics::LaunchSettings {
      safe,
      no_jev: false,
    };
    crate::epics::write_launch_settings(&scenario.state, settings).expect("safe");
    let mut engine = scenario.engine();
    assert_eq!(engine.pump().expect("pump"), Stop::Idle);
    assert_eq!(stage(&engine, &key(2)), "running");
    let record = scenario.record(2);
    assert_eq!(record["bypass"], json!(!safe), "{record}");
    let started: Vec<Value> = scenario
      .herdr
      .calls()
      .into_iter()
      .filter(|(method, _)| method == "agent.start")
      .map(|(_, params)| params["args"].clone())
      .collect();
    let bypass = json!("--dangerously-skip-permissions");
    assert_eq!(started.len(), 1, "{started:?}");
    let args = started[0].as_array().cloned().unwrap_or_default();
    assert_eq!(args.contains(&bypass), !safe, "{args:?}");
  }
}
