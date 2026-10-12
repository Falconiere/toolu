use super::commit_effect;
use crate::epics::Member;
use crate::journal::Record;
use crate::logic::commit_journal;
use crate::model::{Action, Step, World, fresh};
use crate::refill_rig::world;

/// Issue `b` of a two-member batch at `stage`, with a launch `t` in flight.
fn launching(stage: &str, reroute: bool) -> World {
  let mut world = world(&[("a", 2, 0), ("b", 1, 0)], 2);
  if let Some(epic) = world.epics.get_mut("e") {
    epic.batch = ["a", "b"]
      .iter()
      .map(|key| Member {
        key: (*key).to_owned(),
        outcome: None,
      })
      .collect();
  }
  let issue = world.issues.get_mut("b").expect("b");
  let mut pending = fresh(Action::Launch, "t".into());
  pending.phase = 1;
  pending.reroute = reroute;
  issue.pending = Some(pending);
  stage.clone_into(&mut issue.stage);
  world
}

fn done_line(steps: &[Step]) -> Option<Record> {
  steps.iter().find_map(|step| match step {
    Step::Journal(row) if row.name == "launch-done" => Some(row.clone()),
    Step::Journal(_)
    | Step::Call { .. }
    | Step::Status { .. }
    | Step::Issue { .. }
    | Step::Prompt { .. }
    | Step::Notify { .. } => None,
  })
}

#[test]
fn a_held_waiting_or_failed_launch_closes_its_intent_with_a_note() {
  for (doc, note) in [
    (
      r#"{"outcome":"hold","with":"a","score":0.9,"source":"jev"}"#,
      "hold",
    ),
    (r#"{"outcome":"wait","note":"no host capacity"}"#, "wait"),
    (r#"{"outcome":"failed","note":"bad brief"}"#, "failed"),
    ("failed: effect panicked", "failed"),
  ] {
    let mut world = launching("", false);
    let steps = commit_effect(&mut world, "b", doc);
    let line = done_line(&steps).expect(doc);
    assert_eq!(
      (line.token.as_str(), line.note.as_str()),
      ("t", note),
      "{doc}"
    );
    assert!(world.issues["b"].pending.is_none(), "{doc}");
    assert_eq!(commit_journal(&mut world, &line), Vec::new(), "{doc}");
  }
}

#[test]
fn a_panicked_launch_fails_like_a_refused_one_and_a_panicked_cleanup_is_incomplete() {
  let mut world = launching("", false);
  let _steps = commit_effect(&mut world, "b", "failed: effect panicked");
  assert_eq!(world.issues["b"].deferred.as_deref(), Some("launch"));
  assert_eq!(world.epics["e"].batch[1].outcome.as_deref(), Some("failed"));
  assert_eq!(world.attention[0].kind, "failed");
  assert_eq!(world.attention[0].note, "failed: effect panicked");
  let issue = world.issues.get_mut("a").expect("a");
  issue.stage = "cleaning".into();
  issue.pending = Some(fresh(Action::Cleanup, "c".into()));
  let steps = commit_effect(&mut world, "a", "failed: effect thread stopped");
  assert_eq!(world.issues["a"].stage, "cleanup-incomplete");
  assert!(world.issues["a"].pending.is_none());
  assert!(
    steps
      .iter()
      .any(|step| matches!(step, Step::Issue { key } if key == "a"))
  );
  assert_eq!(world.attention[1].kind, "cleanup-incomplete");
}

#[test]
fn a_relaunch_or_reroute_that_waits_is_marked_and_rearmed_after_its_retry_time() {
  for (reroute, marker) in [(false, "lost"), (true, "reroute")] {
    let mut world = launching("running", reroute);
    let steps = commit_effect(&mut world, "b", r#"{"outcome":"wait","note":"full"}"#);
    assert!(
      steps
        .iter()
        .any(|step| matches!(step, Step::Issue { key } if key == "b"))
    );
    let issue = &world.issues["b"];
    assert!(issue.pending.is_none(), "{marker}");
    assert_eq!(issue.relaunch.as_deref(), Some(marker));
    let retry = issue.retry_at_ms;
    assert_eq!(retry, world.now_ms + 30_000);
    // `a` already runs, so the only launch refill can arm is `b`'s relaunch.
    world.issues.get_mut("a").expect("a").stage = "running".into();
    crate::refill::refill(&mut world);
    assert!(world.issues["b"].pending.is_none(), "{marker}");
    world.now_ms = retry;
    crate::refill::refill(&mut world);
    let pending = world.issues["b"].pending.clone().expect("rearmed");
    assert_eq!((pending.action, pending.reroute), (Action::Launch, reroute));
    let _steps = commit_effect(&mut world, "b", r#"{"outcome":"applied","kind":"codex"}"#);
    assert_eq!(world.issues["b"].relaunch, None);
  }
}

#[test]
fn a_fresh_launch_that_waits_leaves_its_batch() {
  let mut world = launching("", false);
  let _steps = commit_effect(&mut world, "b", r#"{"outcome":"wait","note":"full"}"#);
  assert!(world.issues["b"].pending.is_none());
  assert_eq!(world.epics["e"].batch[1].outcome.as_deref(), Some("wait"));
}
