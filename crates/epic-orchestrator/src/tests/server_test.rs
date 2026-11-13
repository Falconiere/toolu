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
  assert_eq!(engine.tick(&()).expect("tick"), Stop::Idle);
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
  assert_eq!(engine.tick(&()).expect("tick"), Stop::Idle);
  assert!(engine.world.issues.contains_key("a"));
}

#[test]
fn a_tick_moves_the_clock_off_its_open_time() {
  let tmp = tempfile::tempdir().expect("temp");
  let mut engine = Engine::open(Paths::at(tmp.path()), None, Fault::None).expect("open");
  engine.world.now_ms = 1;
  assert_eq!(engine.tick(&()).expect("tick"), Stop::Idle);
  assert!(engine.world.now_ms > 1_000_000_000_000);
}

fn prompt(key: &str, fallback: Option<&str>) -> crate::model::Step {
  crate::model::Step::Prompt {
    key: key.to_owned(),
    text: "fix the thread".to_owned(),
    fallback: fallback.map(str::to_owned),
  }
}

#[test]
fn an_undelivered_prompt_raises_its_fallback_attention_item() {
  let tmp = tempfile::tempdir().expect("temp");
  let mut engine = Engine::open(Paths::at(tmp.path()), None, Fault::None).expect("open");
  engine.world.outbox.push_back(prompt("a", Some("babysit")));
  engine.world.outbox.push_back(prompt("b", None));
  assert_eq!(engine.pump().expect("pump"), Stop::Idle);
  let raised: Vec<(&str, &str, &str)> = engine
    .world
    .attention
    .iter()
    .map(|item| (item.kind.as_str(), item.key.as_str(), item.note.as_str()))
    .collect();
  assert_eq!(
    raised,
    [(
      "babysit",
      "a",
      "worker prompt not delivered: prompt deferred"
    )]
  );
}

#[test]
fn a_prompt_script_gets_the_key_and_text_and_its_outcome_counts() {
  let tmp = tempfile::tempdir().expect("temp");
  let scripts = tmp.path().join("scripts");
  std::fs::create_dir_all(&scripts).expect("scripts");
  let seen = tmp.path().join("seen.json");
  let script = format!(
    "cat > '{}'\nprintf '{{\"outcome\":\"sent\"}}'\n",
    seen.display()
  );
  std::fs::write(scripts.join("prompt"), script).expect("script");
  let mut engine = Engine::open(Paths::at(tmp.path()), Some(scripts), Fault::None).expect("open");
  engine.world.outbox.push_back(prompt("a", Some("babysit")));
  assert_eq!(engine.pump().expect("pump"), Stop::Idle);
  assert_eq!(engine.world.attention.len(), 0);
  let body: serde_json::Value =
    serde_json::from_str(&std::fs::read_to_string(seen).expect("seen")).expect("json");
  assert_eq!(
    body,
    serde_json::json!({"key": "a", "text": "fix the thread"})
  );
}
