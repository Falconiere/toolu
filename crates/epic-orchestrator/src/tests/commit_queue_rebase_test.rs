use super::commit;
use crate::model::{Action, Issue, Step, World, fresh};
use crate::queue_fields::Held;

#[test]
fn an_uncertain_rebase_push_is_held_without_a_worker_prompt() {
  let mut world = World::new(1_000);
  for (key, at) in [("a", 10), ("b", 20)] {
    let mut issue = Issue::blank(key, "e", "/state", 0);
    issue.phase = "ready".into();
    issue.stage = "running".into();
    issue.repo = "o/r".into();
    issue.pr = Some(if key == "a" { 1 } else { 2 });
    issue.queue.queued_at_ms = Some(at);
    world.issues.insert(key.into(), issue);
  }
  let a = world.issues.get_mut("a").expect("a");
  a.pending = Some(fresh(Action::Merge, "t1".into()));
  a.queue.held = Some(Held {
    prompt: "fix the thread".into(),
    note: None,
  });
  let steps = commit(
    &mut world,
    "a",
    r#"{"outcome":"rebase-uncertain","note":"remote inspection failed"}"#,
  )
  .expect("doc");
  let a = &world.issues["a"];
  assert_eq!(a.deferred.as_deref(), Some("rebase"));
  assert_eq!(a.phase, "ready");
  assert!(a.pending.is_none());
  assert!(a.queue.held.is_some(), "babysit finding stays held");
  assert_eq!(
    world.attention[0].note,
    "rebase push uncertain: remote inspection failed"
  );
  assert!(steps.iter().any(|step| matches!(step, Step::Journal(row) if row.name == "merge-done" && row.note == "rebase-uncertain: remote inspection failed")));
  assert!(
    steps
      .iter()
      .any(|step| matches!(step, Step::Issue { key } if key == "a"))
  );
  assert!(!steps.iter().any(|step| matches!(step, Step::Prompt { .. })));
  crate::logic::arm_ready(&mut world);
  assert!(world.issues["a"].pending.is_none(), "no second queue step");
  assert!(world.issues["b"].pending.is_none(), "the head remains held");
}
