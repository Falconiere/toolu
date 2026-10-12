use serde_json::json;

use super::{blocker_keys, load_epic_state, max_parallel};
use crate::model::World;

#[test]
fn a_full_graph_gives_the_epic_its_reference_limit_and_graph() {
  let mut world = World::new(0);
  let graph = serde_json::to_value(crate::support::graph_248()).expect("graph");
  load_epic_state(&mut world, "comemory-248", "/state", &graph);
  let epic = &world.epics["comemory-248"];
  assert_eq!(
    (epic.reference.as_str(), epic.max_parallel),
    ("Falconiere/comemory#248", 3)
  );
  assert!(epic.graph.is_some());
  assert_eq!(max_parallel(&world, "comemory-248"), 3);
  assert_eq!(max_parallel(&world, "other"), world.max_parallel);
}

#[test]
fn a_minimal_graph_keeps_defaults_and_the_running_batch() {
  let mut world = World::new(0);
  world.max_parallel = 2;
  load_epic_state(&mut world, "one", "/s", &json!({"issues": [{"key": "a"}]}));
  world
    .epics
    .get_mut("one")
    .expect("one")
    .batch
    .push(crate::epics::Member {
      key: "a".into(),
      outcome: None,
    });
  load_epic_state(
    &mut world,
    "one",
    "/s",
    &json!({"issues": [{"key": "a"}], "max_parallel": 0}),
  );
  let epic = &world.epics["one"];
  assert_eq!(
    (epic.max_parallel, epic.graph.is_none(), epic.batch.len()),
    (1, true, 1)
  );
}

#[test]
fn blockers_named_by_reference_become_issue_keys() {
  let graph = json!({"issues": [{"ref": "o/r#1", "key": "r-1"}, {"ref": "o/r#2", "key": "r-2"}]});
  assert_eq!(
    blocker_keys(&graph, &["o/r#1".into(), "x/y#9".into(), "r-2".into()]),
    ["r-1", "x/y#9", "r-2"]
  );
}

#[test]
fn launch_settings_default_off_follow_start_and_turn_on_when_damaged() {
  use super::{LaunchSettings, launch_settings, write_launch_settings};
  let tmp = tempfile::tempdir().expect("temp");
  let on = |safe: bool, no_jev: bool| LaunchSettings { safe, no_jev };
  assert_eq!(launch_settings(tmp.path()), on(false, false));
  write_launch_settings(tmp.path(), on(true, false)).expect("safe");
  assert_eq!(launch_settings(tmp.path()), on(true, false));
  write_launch_settings(tmp.path(), on(false, true)).expect("no jev");
  assert_eq!(launch_settings(tmp.path()), on(false, true));
  for (text, expected) in [
    ("{", on(true, true)),
    ("", on(true, true)),
    ("null", on(true, true)),
    ("[]", on(true, true)),
    ("{}", on(false, false)),
    (r#"{"safe":"true","no_jev":false}"#, on(true, false)),
    (r#"{"safe":false,"no_jev":1}"#, on(false, true)),
  ] {
    std::fs::write(tmp.path().join("launch.json"), text).expect("settings");
    assert_eq!(launch_settings(tmp.path()), expected, "{text}");
  }
}
