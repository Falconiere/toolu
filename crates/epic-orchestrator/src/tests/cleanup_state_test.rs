use super::*;
use crate::model::{Action, QueuedPrompt, fresh};

#[test]
fn safety_refuses_active_pending_and_incomplete_issues() {
  let mut world = World::new(0);
  let mut issue = Issue::blank("a", "one", "/state", 0);
  for stage in [
    "starting",
    "running",
    "awaiting_merge",
    "replacing",
    "uncertain",
    "cleaning",
    "cleanup-incomplete",
  ] {
    issue.stage = stage.into();
    world.issues.insert("a".into(), issue.clone());
    assert!(safety(&world, "one").is_some(), "{stage}");
  }
  issue.stage = "merged".into();
  issue.pending = Some(fresh(Action::Cleanup, "t".into()));
  world.issues.insert("a".into(), issue.clone());
  assert_eq!(safety(&world, "one"), Some("pending worker action"));
  issue.pending = None;
  world.issues.insert("a".into(), issue);
  assert_eq!(safety(&world, "one"), None);
}

#[test]
fn prune_removes_related_prompts_attention_and_scheduling_only() {
  let tmp = tempfile::tempdir().unwrap();
  let mut engine = Engine::open(Paths::at(tmp.path()), None, crate::server::Fault::None).unwrap();
  for key in ["one", "two"] {
    engine
      .world
      .epics
      .insert(key.into(), crate::epics::Epic::new(key, "/s", 0, None));
    engine.world.epic_refs.insert(key.into(), "o/r#1".into());
    engine.world.paused.insert(key.into());
    let worker = format!("{key}-a");
    engine
      .world
      .issues
      .insert(worker.clone(), Issue::blank(&worker, key, "/s", 0));
    engine.world.prompt_queue.insert(
      worker.clone(),
      QueuedPrompt {
        text: "hi".into(),
        due_ms: 0,
        name: "unblocked".into(),
        fallback: None,
      },
    );
    engine.world.relayed_at_ms.insert(worker.clone(), 0);
    crate::attention::raise(&mut engine.world, "failed", &worker, "note");
    engine.world.outbox.push_back(Step::Prompt {
      key: worker,
      text: "hi".into(),
      fallback: None,
    });
  }
  prune(&mut engine, &BTreeSet::from(["one".into()]));
  assert_eq!(engine.world.epics.keys().collect::<Vec<_>>(), ["two"]);
  assert_eq!(engine.world.issues.keys().collect::<Vec<_>>(), ["two-a"]);
  assert_eq!(engine.world.attention.len(), 1);
  assert_eq!(
    engine.world.prompt_queue.keys().collect::<Vec<_>>(),
    ["two-a"]
  );
  assert_eq!(
    engine.world.relayed_at_ms.keys().collect::<Vec<_>>(),
    ["two-a"]
  );
  assert_eq!(engine.world.outbox.len(), 3);
}
