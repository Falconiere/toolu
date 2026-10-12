use serde_json::json;

use super::{coupling_ahead, hold, release};
use crate::epics::Member;
use crate::refill_rig::{armed, journaled, world};

fn members(world: &mut crate::model::World, rows: &[(&str, Option<&str>)]) {
  world.epics.get_mut("e").expect("epic").batch = rows
    .iter()
    .map(|(key, outcome)| Member {
      key: (*key).into(),
      outcome: outcome.map(str::to_owned),
    })
    .collect();
}

#[test]
fn a_hold_records_the_partner_raises_a_judgment_and_journals() {
  let mut world = world(&[("a", 2, 0), ("b", 1, 0)], 2);
  members(&mut world, &[("a", Some("applied")), ("b", None)]);
  let steps = hold(&mut world, "b", "a", &json!(0.93), "jev");
  assert_eq!(
    world.issues["b"]
      .hold
      .as_ref()
      .map(|hold| hold["with"].clone()),
    Some(json!("a"))
  );
  assert!(
    world
      .attention
      .iter()
      .any(|item| item.kind == "coupling-hold" && item.note.contains("held behind a"))
  );
  assert!(steps.iter().any(|step| matches!(step, crate::model::Step::Journal(record) if record.name == "hold" && record.note == "a")));
  assert_eq!(world.epics["e"].batch[1].outcome.as_deref(), Some("hold"));
}

#[test]
fn release_clears_the_hold_and_journals() {
  let mut world = world(&[("a", 1, 0)], 1);
  let _steps = hold(&mut world, "a", "z", &json!(1.0), "heuristic");
  world.outbox.clear();
  release(&mut world, "a").expect("release");
  assert!(world.issues["a"].hold.is_none() && world.issues["a"].coupling_released);
  assert_eq!(
    journaled(&world),
    [
      (
        "closed".to_owned(),
        "kind=coupling-hold waited_ms=0 by=release".to_owned()
      ),
      ("release".to_owned(), "z".to_owned())
    ]
  );
  assert!(
    world.attention.is_empty(),
    "the release settles the hold's judgment"
  );
}

#[test]
fn release_refuses_an_unknown_or_unheld_issue() {
  let mut world = world(&[("a", 1, 0)], 1);
  assert_eq!(
    release(&mut world, "nope"),
    Err("no registered issue nope".to_owned())
  );
  assert_eq!(release(&mut world, "a"), Err("a is not held".to_owned()));
}

#[test]
fn only_applied_or_waiting_members_ahead_are_compared() {
  let mut world = world(
    &[
      ("a", 4, 0),
      ("b", 3, 0),
      ("c", 2, 0),
      ("d", 1, 0),
      ("e2", 0, 0),
    ],
    5,
  );
  members(
    &mut world,
    &[
      ("a", Some("applied")),
      ("b", Some("hold")),
      ("c", Some("wait")),
      ("d", None),
      ("e2", None),
    ],
  );
  assert_eq!(coupling_ahead(&world, "d"), ["a", "c"]);
  assert_eq!(coupling_ahead(&world, "a"), Vec::<String>::new());
  world.issues.get_mut("d").expect("d").coupling_released = true;
  assert_eq!(coupling_ahead(&world, "d"), Vec::<String>::new());
}

#[test]
fn a_relaunch_or_an_issue_outside_the_batch_is_never_compared() {
  let mut world = world(&[("a", 3, 0), ("b", 2, 0), ("c", 1, 0)], 3);
  members(&mut world, &[("a", Some("applied")), ("b", None)]);
  assert_eq!(coupling_ahead(&world, "b"), ["a"]);
  assert_eq!(coupling_ahead(&world, "c"), Vec::<String>::new());
  world.issues.get_mut("b").expect("b").stage = "running".into();
  assert_eq!(coupling_ahead(&world, "b"), Vec::<String>::new());
}

#[test]
fn a_hold_outcome_from_the_launch_effect_holds_the_issue() {
  let mut world = world(&[("a", 2, 0), ("b", 1, 0)], 2);
  members(&mut world, &[("a", Some("applied")), ("b", None)]);
  world.issues.get_mut("b").expect("b").pending = Some(crate::model::fresh(
    crate::model::Action::Launch,
    "t".into(),
  ));
  let _steps = crate::commit::commit_effect(
    &mut world,
    "b",
    r#"{"outcome":"hold","with":"a","score":0.93,"source":"jev","fallback":null}"#,
  );
  assert!(world.issues["b"].hold.is_some() && world.issues["b"].pending.is_none());
}

#[test]
fn a_released_issue_launches_next_without_the_check_and_then_checks_again() {
  let mut world = world(&[("a", 1, 0)], 1);
  let _steps = hold(&mut world, "a", "z", &json!(1.0), "heuristic");
  release(&mut world, "a").expect("release");
  crate::refill::refill(&mut world);
  assert_eq!(armed(&world), ["a"]);
  let _steps = crate::commit::commit_effect(
    &mut world,
    "a",
    r#"{"outcome":"applied","fallback":"TYPESAFE_API_KEY not set"}"#,
  );
  assert!(!world.issues["a"].coupling_released);
}
