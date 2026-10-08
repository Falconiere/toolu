use super::{Cause, Kind, Watch};
use serde_json::json;

use crate::disk::write_value;
use crate::paths::Paths;
use crate::server::{Engine, Fault};

fn pr() -> Kind {
  Kind::Pr {
    key: "toolu-447".to_owned(),
    repo: "Falconiere/toolu".to_owned(),
    number: 501,
  }
}

#[test]
fn github_cadence_stays_anchored_for_six_hours_and_immediate_checks() {
  let mut watch = Watch::new(pr(), 0);
  assert_eq!(watch.take_due(0), Some(Cause::Immediate));
  assert_eq!(watch.take_due(60_000), None);
  for slot in 1..=120 {
    let at = slot * 180_000;
    assert_eq!(watch.take_due(at - 1), None);
    assert_eq!(watch.take_due(at), Some(Cause::Scheduled));
    assert_eq!(watch.last_at_ms, Some(at));
    assert_eq!(watch.next_at_ms, at + 180_000);
    if slot == 30 {
      watch.request_immediate();
      assert_eq!(watch.take_due(at + 1), Some(Cause::Immediate));
      assert_eq!(watch.next_at_ms, at + 180_000);
    }
  }
}

#[test]
fn github_cadence_survives_restart_and_rate_limit_hold() {
  let mut watch = Watch::new(pr(), 0);
  assert_eq!(watch.take_due(0), Some(Cause::Immediate));
  assert_eq!(watch.take_due(180_000), Some(Cause::Scheduled));
  let saved = serde_json::to_vec(&watch).expect("serialize");
  let mut restored: Watch = serde_json::from_slice(&saved).expect("deserialize");
  restored.retry_after_until_ms = 240_000;
  assert_eq!(restored.take_due(220_000), None);
  assert_eq!(restored.take_due(359_999), None);
  assert_eq!(restored.take_due(360_000), Some(Cause::Scheduled));
  assert_eq!(restored.take_due(540_000), Some(Cause::Scheduled));
  assert_eq!(restored.due_at_ms(541_000), 720_000);
}

#[test]
fn github_cadence_resource_kinds_round_trip() {
  let kinds = [
    Kind::Base {
      repo: "Falconiere/toolu".to_owned(),
      branch: "main".to_owned(),
    },
    Kind::Epic {
      key: "falconiere-toolu-402".to_owned(),
      repo: "Falconiere/toolu".to_owned(),
      number: 402,
    },
    Kind::Blocker {
      key: "toolu-447".to_owned(),
      repo: "Falconiere/toolu".to_owned(),
      number: 460,
    },
  ];
  for kind in kinds {
    let bytes = serde_json::to_vec(&kind).expect("encode kind");
    let again: Kind = serde_json::from_slice(&bytes).expect("decode kind");
    assert_eq!(again, kind);
  }
}

#[test]
fn github_watch_discovers_registered_pr_epic_and_blocker_and_restores_deadline() {
  let tmp = tempfile::tempdir().expect("temporary root");
  let paths = Paths::at(tmp.path());
  let state = tmp.path().join("epic");
  registered(&paths, &state);
  let engine = Engine::open(paths.clone(), None, Fault::None).expect("engine");
  assert_eq!(engine.world.watches.len(), 3);
  let before = engine
    .world
    .watches
    .get("pr:Falconiere/toolu#501")
    .expect("PR watch")
    .next_at_ms;
  engine.save_watch().expect("watch snapshot");
  let restored = Engine::open(paths, None, Fault::None).expect("restart");
  assert_eq!(
    restored
      .world
      .watches
      .get("pr:Falconiere/toolu#501")
      .expect("restored PR")
      .next_at_ms,
    before
  );
  assert!(
    restored
      .world
      .watches
      .contains_key("epic:Falconiere/toolu#402")
  );
  assert!(
    restored
      .world
      .watches
      .contains_key("blocker:Falconiere/toolu#460")
  );
}

fn registered(paths: &Paths, state: &std::path::Path) {
  let graph = json!({
    "epic": {"ref": "Falconiere/toolu#402"},
    "issues": [{
      "key": "toolu-447", "repo": "Falconiere/toolu", "number": 447,
      "open_blockers": ["Falconiere/toolu#460"]
    }]
  });
  write_value(&state.join("graph.json"), &graph).expect("graph");
  write_value(
    &state.join("status/toolu-447.json"),
    &json!({"phase": "babysit", "pr": 501}),
  )
  .expect("status");
  write_value(
    &paths.registry(),
    &json!({"epics": [{"key": "falconiere-toolu-402", "state_dir": state}]}),
  )
  .expect("registry");
}

#[test]
fn github_idle_has_no_watches_or_requests_after_ten_virtual_minutes() {
  let tmp = tempfile::tempdir().expect("temporary root");
  let paths = Paths::at(tmp.path());
  let mut engine = Engine::open(paths, None, Fault::None).expect("engine");
  engine.world.now_ms = engine.world.now_ms.saturating_add(600_000);
  super::sync(&mut engine.world);
  super::take_due(&mut engine.world);
  assert!(engine.world.watches.is_empty());
  assert!(engine.world.outbox.is_empty());
}
