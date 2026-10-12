use crate::scenario_rig::{Scenario, key};
use crate::server::{Engine, Stop};

fn stage(engine: &Engine, number: u64) -> String {
  let keys: Vec<&String> = engine.world.issues.keys().collect();
  engine
    .world
    .issues
    .get(&key(number))
    .unwrap_or_else(|| panic!("{} not in {keys:?}", key(number)))
    .stage
    .clone()
}

#[test]
fn scenario_chain_launches_d_after_c_merges_and_closes_the_epic() {
  let scenario = Scenario::new(&[(2, &[]), (3, &[]), (4, &[]), (5, &[4])], 3);
  let mut engine = scenario.engine();
  assert_eq!(engine.pump().expect("pump"), Stop::Idle);
  for number in [2, 3, 4] {
    assert_eq!(
      stage(&engine, number),
      "running",
      "{:?}",
      scenario.journal()
    );
  }
  assert_eq!(stage(&engine, 5), "");
  for number in [2, 3] {
    scenario.close(number);
    scenario.report(&mut engine, number, "ready");
    assert_eq!(
      stage(&engine, number),
      "merged",
      "{:?}",
      engine.world.attention
    );
  }
  assert_eq!(stage(&engine, 5), "", "D waits for C");
  scenario.close(4);
  scenario.report(&mut engine, 4, "ready");
  assert_eq!(stage(&engine, 5), "running", "C merged, so D launches");
  scenario.close(5);
  scenario.report(&mut engine, 5, "ready");
  assert_closed(&scenario, &engine);
}

/// Every child merged, and the epic closed once with no `wait` caller.
fn assert_closed(scenario: &Scenario, engine: &Engine) {
  for number in [2, 3, 4, 5] {
    assert_eq!(scenario.record(number)["stage"], "merged");
  }
  let journal = scenario.journal();
  assert!(
    journal.iter().any(|row| row.name == "epic-closed"),
    "{journal:?}"
  );
  assert!(journal.iter().all(|row| row.name != "wait"));
  let kinds: Vec<&str> = engine
    .world
    .attention
    .iter()
    .map(|item| item.kind.as_str())
    .collect();
  assert_eq!(kinds, ["epic-complete"]);
  let requests = scenario.github.requests().expect("requests");
  let closed = requests.iter().filter(|request| {
    request.method == "PATCH"
      && request.path == "/repos/o/r/issues/1"
      && String::from_utf8_lossy(&request.body).contains("\"closed\"")
  });
  assert_eq!(closed.count(), 1);
}

#[test]
fn scenario_cooldown_routes_to_codex_and_moves_a_limited_worker_in_its_worktree() {
  let scenario = Scenario::new(&[(2, &[])], 1);
  scenario.pool(&["claude", "codex"]);
  let mut engine = scenario.engine();
  assert_eq!(engine.pump().expect("pump"), Stop::Idle);
  assert_eq!(scenario.record(2)["kind"], "claude");
  let worktree = scenario.record(2)["worktree"].clone();
  scenario.report_note(
    &mut engine,
    2,
    "failed",
    "rate-limited: usage limit reached",
  );
  let record = scenario.record(2);
  assert_eq!(
    (record["kind"].as_str(), &record["worktree"]),
    (Some("codex"), &worktree)
  );
  assert_eq!(
    engine.world.attention.len(),
    0,
    "{:?}",
    engine.world.attention
  );
  let journal = scenario.journal();
  for name in ["host-limited", "reroute"] {
    assert!(
      journal.iter().any(|row| row.name == name),
      "{name}: {journal:?}"
    );
  }
  let hosts = crate::disk::read_value(&scenario.state.join("hosts.json")).expect("hosts");
  assert!(hosts["claude"]["until"].is_string(), "{hosts}");
  let cooled = Scenario::new(&[(3, &[])], 1);
  cooled.pool(&["claude", "codex"]);
  std::fs::write(cooled.state.join("hosts.json"), hosts.to_string()).expect("cooldown");
  let mut engine = cooled.engine();
  assert_eq!(engine.pump().expect("pump"), Stop::Idle);
  assert_eq!(cooled.record(3)["kind"], "codex");
}

#[test]
fn scenario_gone_relaunches_twice_then_raises_relaunch_limit_after_a_reroute() {
  let scenario = Scenario::new(&[(2, &[])], 1);
  scenario.pool(&["claude", "codex"]);
  let mut engine = scenario.engine();
  assert_eq!(engine.pump().expect("pump"), Stop::Idle);
  scenario.report_note(
    &mut engine,
    2,
    "failed",
    "rate-limited: usage limit reached",
  );
  assert_eq!(
    engine.world.issues[&key(2)].launches,
    0,
    "a reroute is not a relaunch"
  );
  for expected in [1, 2] {
    scenario.lose_agent(2);
    crate::schedule::note_event(&mut engine.world, &key(2), "gone");
    assert_eq!(engine.pump().expect("pump"), Stop::Idle);
    assert_eq!(engine.world.issues[&key(2)].launches, expected);
    assert_eq!(stage(&engine, 2), "running", "{:?}", engine.world.attention);
  }
  scenario.lose_agent(2);
  crate::schedule::note_event(&mut engine.world, &key(2), "gone");
  assert_eq!(engine.pump().expect("pump"), Stop::Idle);
  let kinds: Vec<&str> = engine
    .world
    .attention
    .iter()
    .map(|item| item.kind.as_str())
    .collect();
  assert_eq!(kinds, ["relaunch-limit"]);
  let checkpoints = scenario
    .journal()
    .iter()
    .filter(|row| row.name == "checkpoint-done")
    .count();
  assert_eq!(checkpoints, 2);
}

