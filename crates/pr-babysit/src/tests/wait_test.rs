use std::cell::{Cell, RefCell};
use std::path::Path;

use serde_json::{Value, json};
use tempfile::TempDir;
use toolu_http_test_support::Fixture;

use super::{Clock, Plan, SystemClock, default_seconds, needs_agent, wait};
use crate::error::{Failure, Result};
use crate::schedule::iso;
use crate::tick::{Request, Source, tick};
use crate::{fixture, recorded};

/// 2026-10-11T12:00:00Z.
const T0: u64 = 1_791_720_000;

struct Virtual {
  now: Cell<u64>,
  overshoot: Cell<u64>,
}

impl Virtual {
  fn at(now: u64) -> Self {
    Virtual {
      now: Cell::new(now),
      overshoot: Cell::new(0),
    }
  }
}

impl Clock for Virtual {
  fn now(&self) -> u64 {
    self.now.get()
  }

  fn sleep_until(&self, at: u64) {
    if at > self.now.get() {
      self.now.set(at + self.overshoot.take());
    }
  }
}

fn pending(fixture: &Fixture) {
  let checks = [recorded::check_run("rust", "IN_PROGRESS", None)];
  let verdict = fixture::text("verdicts/in-progress.txt");
  recorded::serve_open(fixture, &checks, &verdict, "2026-10-11T12:00:00Z");
}

fn green(fixture: &Fixture) {
  let checks = [recorded::check_run("rust", "COMPLETED", Some("SUCCESS"))];
  let verdict = fixture::text("verdicts/pr122-verdict-approved.txt");
  recorded::serve_open(fixture, &checks, &verdict, "2026-10-11T13:00:00Z");
}

/// A scheduled live tick of PR 115 at each checked time, the times recorded.
fn ticker<'a>(
  fixture: &'a Fixture,
  state: &'a Path,
  seen: &'a RefCell<Vec<u64>>,
) -> impl FnMut(u64) -> Result<Value> + 'a {
  let env = recorded::env(fixture);
  move |now| {
    seen.borrow_mut().push(now);
    let at = iso(now);
    let request = Request {
      repo: "Falconiere/toolu",
      number: 115,
      state_file: state,
      snapshot_out: None,
      now: &at,
      scheduled: true,
    };
    let source = Source::GitHub {
      config: recorded::config(fixture),
      page_size: 100,
    };
    tick(&request, source, &env).map(|ticked| ticked.result)
  }
}

fn plan(max_seconds: u64) -> Plan<'static> {
  Plan {
    max_seconds,
    next_check: None,
    slot: "falconiere-toolu-115",
    state_path: "/s.json",
    result_path: "/s.result.json",
  }
}

#[test]
fn a_pending_pr_waits_exactly_the_per_host_default() {
  for (host, seconds, checks) in [("claude", 1800, 11), ("codex", 55, 1), ("opencode", 55, 1)] {
    let dir = TempDir::new().unwrap();
    let fixture = Fixture::start().unwrap();
    pending(&fixture);
    let seen = RefCell::new(Vec::new());
    let clock = Virtual::at(T0);
    let state = dir.path().join("s.json");
    let waited = wait(
      &clock,
      &plan(default_seconds(host)),
      &mut ticker(&fixture, &state, &seen),
    );
    let waited = waited.unwrap();
    assert_eq!(waited["state"], "waiting", "{host}");
    assert_eq!(clock.now(), T0 + seconds, "{host}");
    assert_eq!(seen.borrow().len(), checks, "{host}");
    assert_eq!(waited["slot"], "falconiere-toolu-115");
  }
  assert_eq!(default_seconds("hermes"), 55);
}

#[test]
fn zero_seconds_checks_once_then_waits() {
  let dir = TempDir::new().unwrap();
  let fixture = Fixture::start().unwrap();
  pending(&fixture);
  let seen = RefCell::new(Vec::new());
  let clock = Virtual::at(T0);
  let state = dir.path().join("s.json");
  let waited = wait(&clock, &plan(0), &mut ticker(&fixture, &state, &seen)).unwrap();
  assert_eq!(waited["nextCheckAt"], iso(T0 + 180));
  assert_eq!(*seen.borrow(), [T0]);
}

#[test]
fn a_pr_that_turns_green_returns_success_at_the_first_check_after() {
  for (turns, expected) in [(T0 + 400, T0 + 540), (T0 + 360, T0 + 360)] {
    let dir = TempDir::new().unwrap();
    let fixture = Fixture::start().unwrap();
    pending(&fixture);
    let seen = RefCell::new(Vec::new());
    let clock = Virtual::at(T0);
    let state = dir.path().join("s.json");
    let mut live = ticker(&fixture, &state, &seen);
    let mut check = |now: u64| {
      if now >= turns {
        green(&fixture);
      }
      live(now)
    };
    let result = wait(&clock, &plan(1800), &mut check).unwrap();
    assert_eq!(result["decision"], "success", "{:?}", result["reasons"]);
    assert_eq!(clock.now(), expected);
    assert!(expected - turns <= 180);
  }
}

#[test]
fn six_hours_of_pending_ci_check_121_times_180_seconds_apart() {
  let dir = TempDir::new().unwrap();
  let fixture = Fixture::start().unwrap();
  pending(&fixture);
  let seen = RefCell::new(Vec::new());
  let clock = Virtual::at(T0);
  let state = dir.path().join("s.json");
  let waited = wait(&clock, &plan(21_600), &mut ticker(&fixture, &state, &seen)).unwrap();
  assert_eq!(waited["state"], "waiting");
  let seen = seen.borrow();
  assert_eq!(seen.len(), 121);
  assert!(seen.windows(2).all(|pair| pair[1] - pair[0] == 180));
  let graphql = fixture
    .requests()
    .unwrap()
    .iter()
    .filter(|r| r.path == "/graphql")
    .count();
  assert_eq!(graphql, 2 * 121, "the PR and threads queries of each check");
}

#[test]
fn a_late_wake_runs_one_check_and_keeps_the_grid() {
  let dir = TempDir::new().unwrap();
  let fixture = Fixture::start().unwrap();
  pending(&fixture);
  let seen = RefCell::new(Vec::new());
  let clock = Virtual::at(T0);
  let state = dir.path().join("s.json");
  let mut live = ticker(&fixture, &state, &seen);
  let mut check = |now: u64| {
    if now == T0 + 180 {
      clock.overshoot.set(500);
    }
    live(now)
  };
  wait(&clock, &plan(900), &mut check).unwrap();
  assert_eq!(*seen.borrow(), [T0, T0 + 180, T0 + 860, T0 + 900]);
}

#[test]
fn red_ci_returns_at_the_first_check_with_attention() {
  let dir = TempDir::new().unwrap();
  let fixture = Fixture::start().unwrap();
  let checks = [recorded::check_run("rust", "COMPLETED", Some("FAILURE"))];
  let verdict = fixture::text("verdicts/in-progress.txt");
  recorded::serve_open(&fixture, &checks, &verdict, "2026-10-11T12:00:00Z");
  let seen = RefCell::new(Vec::new());
  let clock = Virtual::at(T0);
  let state = dir.path().join("s.json");
  let result = wait(&clock, &plan(1800), &mut ticker(&fixture, &state, &seen)).unwrap();
  assert_eq!(result["attention"]["codes"], json!(["ci_failed"]));
  assert_eq!(result["attention"]["new"], true);
  assert_eq!(clock.now(), T0);
}

#[test]
fn a_scheduled_next_check_in_the_future_is_waited_for_and_errors_stop_the_wait() {
  let clock = Virtual::at(T0);
  let mut plan = plan(600);
  plan.next_check = Some(T0 + 100);
  let mut calls = Vec::new();
  let mut failing = |now: u64| -> Result<Value> {
    calls.push(now);
    Err(Failure::new("api_error", "boom"))
  };
  let failure = wait(&clock, &plan, &mut failing).err().unwrap();
  assert_eq!(failure.code, "api_error");
  assert_eq!(calls, [T0 + 100]);
}

#[test]
fn needs_agent_covers_success_escalation_attention_and_an_active_fixer() {
  assert!(needs_agent(&json!({"decision": "success"})));
  assert!(needs_agent(&json!({"decision": "escalate"})));
  assert!(needs_agent(
    &json!({"decision": "keep_going", "attention": {"needed": true}})
  ));
  assert!(needs_agent(
    &json!({"decision": "keep_going", "reasons": [{"code": "fixer_running"}]})
  ));
  assert!(!needs_agent(
    &json!({"decision": "keep_going", "attention": {"needed": false}})
  ));
}

#[test]
fn the_system_clock_reads_now_and_does_not_sleep_for_the_past() {
  let clock = SystemClock;
  let now = clock.now();
  assert!(now > T0 - 400 * 86_400);
  let started = std::time::Instant::now();
  clock.sleep_until(now.saturating_sub(10));
  clock.sleep_until(clock.now() + 1);
  assert!(started.elapsed() < std::time::Duration::from_secs(3));
}
