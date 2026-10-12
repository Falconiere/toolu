use super::{done, end_wait, note_wait, occupies, ready, refill};
use crate::refill_rig::{armed, batch, journaled, world};

#[test]
fn the_longest_chain_goes_first_then_the_most_unblocking() {
  let mut world = world(&[("z", 1, 9), ("y", 2, 4), ("x", 3, 2)], 3);
  refill(&mut world);
  assert_eq!(batch(&world), ["x", "y", "z"]);
  assert_eq!(armed(&world), ["x"]);
}

#[test]
fn every_occupied_stage_takes_a_slot() {
  let mut world = world(&[("a", 1, 0), ("b", 1, 0), ("c", 1, 0)], 2);
  world.issues.get_mut("a").expect("a").stage = "uncertain".into();
  world.issues.get_mut("b").expect("b").stage = "cleanup-incomplete".into();
  refill(&mut world);
  assert_eq!(armed(&world), Vec::<String>::new());
  assert!(occupies(&world.issues["a"]) && occupies(&world.issues["b"]));
}

#[test]
fn closed_abandoned_and_merged_issues_never_launch() {
  let mut world = world(&[("a", 1, 0), ("b", 1, 0), ("c", 1, 0)], 3);
  for (key, stage) in [("a", "closed"), ("b", "abandoned"), ("c", "merged")] {
    world.issues.get_mut(key).expect("issue").stage = stage.into();
  }
  refill(&mut world);
  assert_eq!(armed(&world), Vec::<String>::new());
  assert!(done(&world.issues["b"]) && done(&world.issues["c"]) && !done(&world.issues["a"]));
  assert!(!occupies(&world.issues["a"]));
}

#[test]
fn a_batch_launches_one_member_at_a_time() {
  let mut world = world(&[("a", 2, 0), ("b", 1, 0)], 2);
  refill(&mut world);
  assert_eq!(
    (batch(&world), armed(&world)),
    (vec!["a".to_owned(), "b".to_owned()], vec!["a".to_owned()])
  );
  refill(&mut world);
  assert_eq!(
    armed(&world),
    ["a"],
    "no second launch while one is pending"
  );
  let _steps = crate::commit::commit_effect(&mut world, "a", r#"{"outcome":"applied"}"#);
  world.issues.get_mut("a").expect("a").pending = None;
  refill(&mut world);
  assert_eq!(armed(&world), ["b"]);
}

#[test]
fn a_blocked_issue_waits_for_its_blocker_to_merge() {
  let mut world = world(&[("a", 2, 1), ("b", 1, 0)], 2);
  world.issues.get_mut("b").expect("b").blockers = vec!["a".into()];
  assert!(!ready(&world, &world.issues["b"]));
  world.issues.get_mut("a").expect("a").stage = "merged".into();
  assert!(ready(&world, &world.issues["b"]));
}

#[test]
fn a_member_that_cannot_launch_any_more_is_skipped() {
  let mut world = world(&[("a", 2, 0), ("b", 1, 0)], 2);
  refill(&mut world);
  world.issues.get_mut("b").expect("b").stage = "running".into();
  let _steps = crate::commit::commit_effect(&mut world, "a", r#"{"outcome":"applied"}"#);
  world.issues.get_mut("a").expect("a").pending = None;
  refill(&mut world);
  assert_eq!(
    world.epics["e"].batch[1].outcome.as_deref(),
    Some("skipped")
  );
  assert_eq!(armed(&world), Vec::<String>::new());
}

#[test]
fn a_paused_epic_forms_no_batch() {
  let mut world = world(&[("a", 1, 0)], 1);
  world.paused.insert("e".into());
  refill(&mut world);
  assert!(batch(&world).is_empty() && armed(&world).is_empty());
}

#[test]
fn the_budget_floor_holds_launches_and_journals_one_wait() {
  let mut world = world(&[("a", 1, 0)], 1);
  world.graphql_remaining = Some(10);
  refill(&mut world);
  assert_eq!(armed(&world), Vec::<String>::new());
  assert_eq!(
    journaled(&world),
    [(
      "launch-wait".to_owned(),
      "GitHub budget under its floor".to_owned()
    )]
  );
  assert!(
    !ready(&world, &world.issues["a"]),
    "the next try waits a tick"
  );
  world.now_ms += 30_000;
  assert!(ready(&world, &world.issues["a"]));
}

#[test]
fn five_waits_journal_once_at_the_start_and_once_with_the_count() {
  let mut world = world(&[("a", 1, 0)], 1);
  for _ in 0..5 {
    note_wait(&mut world, "a", "no host capacity");
  }
  end_wait(&mut world, "a");
  end_wait(&mut world, "a");
  assert_eq!(
    journaled(&world),
    [
      ("launch-wait".to_owned(), "no host capacity".to_owned()),
      ("launch-wait".to_owned(), "5".to_owned())
    ]
  );
  assert_eq!(world.issues["a"].wait_streak, 0);
}

#[test]
fn a_waiting_launch_leaves_the_batch_and_returns_as_a_fresh_candidate() {
  let mut world = world(&[("a", 1, 0)], 1);
  refill(&mut world);
  let _steps = crate::commit::commit_effect(
    &mut world,
    "a",
    r#"{"outcome":"wait","note":"no host capacity"}"#,
  );
  assert_eq!(world.epics["e"].batch[0].outcome.as_deref(), Some("wait"));
  assert!(world.issues["a"].pending.is_none());
  world.now_ms += 30_000;
  refill(&mut world);
  assert_eq!(armed(&world), ["a"]);
}

#[test]
fn a_failed_launch_is_not_retried_until_the_issue_moves() {
  let mut world = world(&[("a", 1, 0)], 1);
  refill(&mut world);
  let _steps = crate::commit::commit_effect(
    &mut world,
    "a",
    r#"{"outcome":"failed","note":"boom","stage":"uncertain"}"#,
  );
  assert_eq!(world.issues["a"].stage, "uncertain");
  assert!(
    world
      .attention
      .iter()
      .any(|item| item.kind == "failed" && item.note == "boom")
  );
  refill(&mut world);
  assert_eq!(armed(&world), Vec::<String>::new());
}
