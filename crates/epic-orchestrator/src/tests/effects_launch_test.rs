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
fn a_first_launch_of_an_issue_done_in_the_tracker_settles_without_launching() {
  let tmp = tempfile::tempdir().expect("temp");
  let mut graph = crate::support::graph_248();
  graph.state_dir = tmp.path().display().to_string();
  for issue in &mut graph.issues {
    if issue.key == "comemory-255" {
      "done".clone_into(&mut issue.status);
    }
  }
  crate::graph::write_graph(&tmp.path().join("graph.json"), &graph).expect("graph");
  let settled = launch(tmp.path(), "comemory-255", &[]);
  assert_eq!(settled["outcome"], "settled");
  assert!(settled["stage"].is_null());
  assert!(!tmp.path().join("issues/comemory-255.json").exists());
}

#[test]
fn a_running_workers_relaunch_waits_when_its_host_is_paused() {
  let tmp = tempfile::tempdir().expect("temp");
  let mut graph = crate::support::graph_248();
  graph.state_dir = tmp.path().display().to_string();
  crate::graph::write_graph(&tmp.path().join("graph.json"), &graph).expect("graph");
  let issues = tmp.path().join("issues");
  std::fs::create_dir_all(&issues).expect("issues");
  std::fs::write(
    issues.join("comemory-255.json"),
    r#"{"stage":"running","kind":"codex","launches":1}"#,
  )
  .expect("record");
  let routes = tmp.path().join("routes");
  std::fs::create_dir_all(&routes).expect("routes");
  let saved = r#"{"key":"comemory-255","host":"codex"}"#;
  std::fs::write(routes.join("comemory-255.json"), saved).expect("route");
  let root = tmp.path().join("r");
  std::fs::create_dir_all(&root).expect("root");
  std::fs::write(
    root.join("state.json"),
    r#"{"version":1,"leases":[],"cooldowns":{},"paused_hosts":{"codex":{"by":"user","at":5}}}"#,
  )
  .expect("state");
  let wait = launch(tmp.path(), "comemory-255", &[]);
  assert_eq!(
    (wait["outcome"].as_str(), wait["note"].as_str()),
    (Some("wait"), Some("host paused"))
  );
  // The saved route survives the wait, so the resume relaunches on codex.
  assert_eq!(
    std::fs::read_to_string(routes.join("comemory-255.json")).expect("route"),
    saved
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

#[test]
fn a_settled_record_is_never_relaunched_even_after_it_left_the_graph() {
  let tmp = tempfile::tempdir().expect("temp");
  let issues = tmp.path().join("issues");
  std::fs::create_dir_all(&issues).expect("issues");
  std::fs::write(issues.join("gone-1.json"), r#"{"stage":"abandoned"}"#).expect("record");
  let dropped = launch(tmp.path(), "gone-1", &[]);
  assert_eq!(
    dropped,
    serde_json::json!({"outcome": "settled", "stage": "abandoned"})
  );
  let mut graph = crate::support::graph_248();
  graph.state_dir = tmp.path().display().to_string();
  crate::graph::write_graph(&tmp.path().join("graph.json"), &graph).expect("graph");
  std::fs::write(issues.join("comemory-255.json"), r#"{"stage":"merged"}"#).expect("record");
  let merged = launch(tmp.path(), "comemory-255", &[]);
  assert_eq!(
    merged,
    serde_json::json!({"outcome": "settled", "stage": "merged"})
  );
  assert!(!tmp.path().join("worktrees").exists());
  assert_eq!(std::fs::read_dir(&issues).expect("issues").count(), 2);
}

#[test]
fn a_record_settled_after_the_first_check_is_refused_under_the_launch_lock() {
  let tmp = tempfile::tempdir().expect("temp");
  let mut graph = crate::support::graph_248();
  graph.state_dir = tmp.path().display().to_string();
  let issues = tmp.path().join("issues");
  std::fs::create_dir_all(&issues).expect("issues");
  let record = issues.join("comemory-255.json");
  std::fs::write(&record, r#"{"stage":"abandoned","launches":1}"#).expect("record");
  let input = input(tmp.path(), "comemory-255", &[], crate::reach::Reach::live());
  let issue = graph.find("comemory-255").expect("issue");
  let outcome = super::launch_routed(&input, &graph, issue, None);
  assert_eq!(
    outcome,
    serde_json::json!({"outcome": "settled", "stage": "abandoned"})
  );
  assert_eq!(
    std::fs::read_to_string(&record).expect("record"),
    r#"{"stage":"abandoned","launches":1}"#
  );
  assert!(!tmp.path().join("launch-locks/comemory-255").exists());
}

#[test]
fn a_launch_waiting_on_an_approval_screen_is_needs_human_with_its_stage() {
  let note = "codex is waiting on its hook review in pane w1:p1: \"18 hooks are new or changed.\"; review and trust the hooks there, then relaunch with --replace";
  let approval = crate::launch::Failure {
    message: note.to_owned(),
    exit_code: None,
    code: Some("approval_pending".to_owned()),
    uncertain: true,
  };
  let stage = |name: &str| (name == "stage").then(|| "uncertain".to_owned());
  assert_eq!(
    super::outcome_of(Err(approval), stage),
    serde_json::json!({"outcome": "needs-human", "note": note, "stage": "uncertain"})
  );
  let lost = crate::launch::Failure::unknown("cannot attest codex session identity");
  assert_eq!(
    super::outcome_of(Err(lost), stage),
    serde_json::json!({"outcome": "failed", "note": "cannot attest codex session identity", "stage": "uncertain"})
  );
}
