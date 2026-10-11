use super::*;
use crate::dispatch::sandbox::{Sandbox, sandbox};

fn with_round(sb: &Sandbox) {
  let mut state = sb.state();
  let group = |seq: u64, host: &str| json!({"seq": seq, "tier": "standard", "host": host, "model": null, "effort": null, "items": ["ci:test"], "status": "running", "reason": null});
  state["fixer"] = json!({"round": 1, "status": "running", "reason": null, "current": 1, "groups": [group(1, "opencode"), group(2, "codex")]});
  std::fs::write(&sb.state, state.to_string()).unwrap();
}

#[test]
fn without_a_round_the_status_is_none() {
  let sb = sandbox();
  let status = Dispatcher::new(&sb.options(false)).unwrap().status();
  assert_eq!(status["status"], "none");
  assert_eq!(status["groups"], json!([]));
  assert_eq!(status["worktree"], Value::Null);
}

#[test]
fn a_settled_group_moves_the_round_on_and_the_last_one_ends_it() {
  let sb = sandbox();
  with_round(&sb);
  let mut dispatcher = Dispatcher::new(&sb.options(false)).unwrap();
  dispatcher.settle(1, "done", "abc123", "").unwrap();
  assert!(dispatcher.next(1, 2).unwrap());
  assert_eq!(sb.state()["fixer"]["current"], 2);
  assert_eq!(sb.state()["fixer"]["groups"][0]["head"], "abc123");
  dispatcher.settle(2, "done", "def456", "").unwrap();
  assert!(!dispatcher.next(2, 2).unwrap());
  assert_eq!(sb.state()["fixer"]["status"], "done");
  let summary = &dispatcher.status()["groups"][1];
  assert_eq!(summary["status"], "done");
  assert!(summary.get("error").is_none());
}

#[test]
fn a_provider_limit_fails_the_round_and_cools_its_host_for_an_hour() {
  let sb = sandbox();
  with_round(&sb);
  let mut dispatcher = Dispatcher::new(&sb.options(false)).unwrap();
  dispatcher
    .settle(1, "host_limited", "", "429 Too Many Requests")
    .unwrap();
  assert!(!dispatcher.next(1, 2).unwrap());
  let state = sb.state();
  assert_eq!(state["fixer"]["status"], "failed");
  assert_eq!(state["fixer"]["reason"], "host_limited");
  assert_eq!(
    state["fixer"]["groups"][0]["error"],
    "429 Too Many Requests"
  );
  let cooldown = &state["hostCooldowns"]["opencode"];
  assert_eq!(cooldown["reason"], "host_limited");
  let until = toolu_state::time::parse_iso(cooldown["until"].as_str().unwrap()).unwrap();
  let left = until.duration_since(SystemTime::now()).unwrap().as_secs();
  assert!((3590..=3600).contains(&left), "{left}");
}

#[test]
fn a_missing_group_is_a_malformed_state_and_a_dry_run_writes_nothing() {
  let sb = sandbox();
  with_round(&sb);
  let failure = Dispatcher::new(&sb.options(false))
    .unwrap()
    .group(9)
    .unwrap_err();
  assert_eq!(
    (failure.code, failure.message.as_str()),
    ("state_malformed", "fixer group 9 is missing")
  );
  let before = std::fs::read_to_string(&sb.state).unwrap();
  let mut dry = Dispatcher::new(&sb.options(true)).unwrap();
  dry.settle(1, "done", "abc", "").unwrap();
  let fetched = dry
    .git(&argv(&["git", "fetch"]), "git_error", String::new())
    .unwrap();
  assert_eq!(fetched, "");
  assert_eq!(dry.commands, vec![argv(&["git", "fetch"])]);
  assert_eq!(std::fs::read_to_string(&sb.state).unwrap(), before);
}
