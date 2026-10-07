use super::{Engine, Fault, Stop};
use crate::paths::Paths;

#[test]
fn an_empty_registry_pumps_idle() {
  let tmp = tempfile::tempdir().expect("temp");
  let mut engine = Engine::open(Paths::at(tmp.path()), None, Fault::None).expect("open");
  assert_eq!(engine.pump().expect("pump"), Stop::Idle);
  assert!(engine.world.issues.is_empty());
}

#[test]
fn a_tick_and_an_absent_herdr_session_record_backoff() {
  let tmp = tempfile::tempdir().expect("temp");
  let mut engine = Engine::open(Paths::at(tmp.path()), None, Fault::None).expect("open");
  engine.herdr_session = Some("toolu-epic-engine-absent".to_owned());
  assert_eq!(engine.tick().expect("tick"), Stop::Idle);
  engine.probe_herdr().expect("probe");
  assert!(engine.world.herdr_failures >= 1);
  assert!(tmp.path().join("watch.json").is_file());
}

#[test]
fn a_tick_adopts_an_epic_registered_after_open() {
  let tmp = tempfile::tempdir().expect("temp");
  let mut engine = Engine::open(Paths::at(tmp.path()), None, Fault::None).expect("open");
  let epic = tmp.path().join("one");
  std::fs::create_dir_all(epic.join("issues")).expect("issues");
  std::fs::write(epic.join("graph.json"), "{\"issues\":[{\"key\":\"a\"}]}").expect("graph");
  let registry = format!(
    "{{\"version\":1,\"epics\":[{{\"key\":\"one\",\"state_dir\":\"{}\"}}]}}\n",
    epic.display()
  );
  std::fs::write(tmp.path().join("registry.json"), registry).expect("registry");
  assert_eq!(engine.tick().expect("tick"), Stop::Idle);
  assert!(engine.world.issues.contains_key("a"));
}

#[test]
fn a_tick_moves_the_clock_off_its_open_time() {
  let tmp = tempfile::tempdir().expect("temp");
  let mut engine = Engine::open(Paths::at(tmp.path()), None, Fault::None).expect("open");
  engine.world.now_ms = 1;
  assert_eq!(engine.tick().expect("tick"), Stop::Idle);
  assert!(engine.world.now_ms > 1_000_000_000_000);
}
