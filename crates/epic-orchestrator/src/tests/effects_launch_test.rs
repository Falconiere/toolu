use serde_json::Value;

use super::{LaunchJob, job, waits};
use crate::paths::Paths;

fn launch(state: &std::path::Path, key: &str, ahead: &[&str]) -> Value {
  launch_with(state, key, ahead, crate::reach::Reach::live())
}

fn launch_with(
  state: &std::path::Path,
  key: &str,
  ahead: &[&str],
  reach: crate::reach::Reach,
) -> Value {
  serde_json::from_str(&job(input(state, key, ahead, reach))()).expect("outcome")
}

fn input(
  state: &std::path::Path,
  key: &str,
  ahead: &[&str],
  reach: crate::reach::Reach,
) -> LaunchJob {
  let env = toolu_runtime::env::Env::from_pairs([
    ("HOME", state.display().to_string()),
    ("TOOLU_RESOURCE_HOME", state.join("r").display().to_string()),
  ]);
  LaunchJob {
    state_dir: state.to_path_buf(),
    key: key.into(),
    ahead: ahead.iter().map(|key| (*key).to_owned()).collect(),
    reroute: false,
    env,
    paths: Paths::at(state.join("engine")),
    reach,
  }
}

#[test]
fn an_unreadable_graph_fails_and_an_overlap_holds_before_routing() {
  let tmp = tempfile::tempdir().expect("temp");
  let failed = launch(tmp.path(), "comemory-255", &[]);
  assert_eq!(failed["outcome"], "failed");
  let mut graph = crate::support::graph_248();
  graph.state_dir = tmp.path().display().to_string();
  for issue in &mut graph.issues {
    if issue.key == "comemory-254" || issue.key == "comemory-255" {
      issue.excerpt = "Adds migration 0042_add_users.".into();
    }
  }
  crate::graph::write_graph(&tmp.path().join("graph.json"), &graph).expect("graph");
  let held = launch(tmp.path(), "comemory-255", &["comemory-254"]);
  assert_eq!(
    (
      held["outcome"].as_str(),
      held["with"].as_str(),
      held["source"].as_str()
    ),
    (Some("hold"), Some("comemory-254"), Some("heuristic"))
  );
  std::fs::write(
    tmp.path().join("pool.json"),
    r#"[{"kind":"claude","cap":1}]"#,
  )
  .expect("pool");
  let crowd = tmp.path().join("issues");
  std::fs::create_dir_all(&crowd).expect("issues");
  std::fs::write(
    crowd.join("other.json"),
    r#"{"stage":"running","kind":"claude"}"#,
  )
  .expect("record");
  let wait = launch(tmp.path(), "comemory-255", &[]);
  assert_eq!(
    (wait["outcome"].as_str(), wait["note"].as_str()),
    (Some("wait"), Some("no host capacity"))
  );
}

#[test]
fn a_first_launch_waits_while_the_github_budget_is_under_its_floor() {
  let tmp = tempfile::tempdir().expect("temp");
  let mut graph = crate::support::graph_248();
  graph.state_dir = tmp.path().display().to_string();
  crate::graph::write_graph(&tmp.path().join("graph.json"), &graph).expect("graph");
  let fixture = toolu_http_test_support::Fixture::start().expect("fixture");
  let budget = serde_json::json!({"resources": {
    "core": {"remaining": 999, "limit": 5000, "reset": 1_900_000_000},
    "graphql": {"remaining": 4000, "limit": 5000, "reset": 1_900_000_000}}});
  crate::support::ok(&fixture, "/rate_limit", &budget);
  let policy =
    crate::retry::Policy::from_env(&toolu_runtime::env::Env::from_pairs([("HOME", "/")]));
  let reach = crate::support::reach(&fixture, policy);
  let wait = launch_with(tmp.path(), "comemory-255", &[], reach);
  assert_eq!(wait["outcome"], "wait");
  let note = wait["note"].as_str().unwrap_or("");
  assert!(
    note.starts_with("GitHub core budget 999/5000 below floor 1000"),
    "{note}"
  );
  assert!(!tmp.path().join("routes").exists());
}

#[test]
fn every_retry_of_one_launch_shares_its_effect_s_sleep_budget() {
  let tmp = tempfile::tempdir().expect("temp");
  let mut graph = crate::support::graph_248();
  graph.state_dir = tmp.path().display().to_string();
  crate::graph::write_graph(&tmp.path().join("graph.json"), &graph).expect("graph");
  let fixture = toolu_http_test_support::Fixture::start().expect("fixture");
  let limited = toolu_http_test_support::Reply::new(429, "slow down").header("Retry-After", "1");
  fixture.route("/rate_limit", limited).expect("route");
  // Each wait (1 s plus the margin) fits the 2 s cap, so without the effect's
  // shared budget the budget read alone would sleep four times.
  let policy = crate::retry::Policy {
    attempts: 5,
    base_ms: 1,
    max_sleep_ms: 2_000,
  };
  let mut job_input = input(
    tmp.path(),
    "comemory-255",
    &[],
    crate::support::reach(&fixture, policy),
  );
  job_input.env = job_input.env.with("EPIC_API_MAX_SLEEP_S", "2");
  let started = std::time::Instant::now();
  let outcome: Value = serde_json::from_str(&job(job_input)()).expect("outcome");
  assert!(
    started.elapsed() < std::time::Duration::from_secs(5),
    "{outcome}"
  );
  let budget_reads = fixture.requests().expect("requests");
  let reads = budget_reads
    .iter()
    .filter(|request| request.path == "/rate_limit")
    .count();
  assert_eq!(reads, 2, "{outcome}");
}

#[test]
fn a_no_jev_epic_never_asks_jev_even_with_a_key() {
  let tmp = tempfile::tempdir().expect("temp");
  let mut job_input = input(tmp.path(), "k-1", &[], crate::reach::Reach::live());
  assert!(!super::jev_allowed(&job_input));
  job_input.env = job_input.env.with("TYPESAFE_API_KEY", "key");
  assert!(super::jev_allowed(&job_input));
  let settings = crate::epics::LaunchSettings {
    safe: false,
    no_jev: true,
  };
  crate::epics::write_launch_settings(tmp.path(), settings).expect("settings");
  assert!(!super::jev_allowed(&job_input));
}

#[test]
fn a_resource_hold_refusal_waits() {
  assert!(waits(
    "resource hold: sustained CPU/load or memory pressure"
  ));
  assert!(waits("job capacity exhausted (1)"));
  assert!(!waits("boom"));
}

#[test]
fn a_busy_pane_refusal_waits() {
  assert!(waits(
    "herdr agent.start: agent_pane_busy: agent target pane wE0:p1 is not an available shell"
  ));
  assert!(!waits(
    "herdr agent.start: timeout: agent did not become ready"
  ));
}
