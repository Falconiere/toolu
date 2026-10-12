use super::{note_babysit, note_change};
use crate::babysit::Next;
use crate::github_probe::Change;
use crate::model::{Action, Issue, World};
use crate::watch::{Kind, Watch};

/// `k` heads `o/r`'s queue with auto-merge armed; its PR is watched.
fn world() -> World {
  let mut world = World::new(1_000);
  let mut issue = Issue::blank("k", "e", "/state", 0);
  "ready".clone_into(&mut issue.phase);
  "running".clone_into(&mut issue.stage);
  "o/r".clone_into(&mut issue.repo);
  issue.pr = Some(7);
  issue.queued_at_ms = Some(1);
  issue.auto_armed = true;
  world.issues.insert("k".to_owned(), issue);
  let kind = Kind::Pr {
    key: "k".to_owned(),
    repo: "o/r".to_owned(),
    number: 7,
  };
  world.watches.insert("pr:o/r#7".to_owned(), Watch::new(kind, 0));
  world
}

#[test]
fn a_babysit_finding_takes_a_queued_issue_back_to_babysit_and_disarms() {
  let mut world = world();
  note_babysit(&mut world, "pr:o/r#7", Next::Prompt("fix the thread".to_owned()));
  let issue = &world.issues["k"];
  assert_eq!((issue.phase.as_str(), issue.queued_at_ms), ("babysit", None));
  assert_eq!(issue.pending.as_ref().map(|p| p.action), Some(Action::Merge), "disarm step");
  assert!(world.outbox.iter().any(|step| matches!(step, crate::model::Step::Prompt { .. })));
  let mut world = self::world();
  note_babysit(&mut world, "pr:o/r#7", Next::Escalate { note: "n".into(), prompt: "p".into() });
  assert_eq!(world.issues["k"].phase, "babysit");
}

#[test]
fn a_failed_tick_stays_queued_and_a_base_move_arms_the_head() {
  let mut world = world();
  note_babysit(&mut world, "pr:o/r#7", Next::Attention("rate limited".to_owned()));
  assert_eq!(world.issues["k"].phase, "ready");
  assert!(world.issues["k"].pending.is_none());
  let moved = Change::BaseMoved {
    repo: "o/r".to_owned(),
    branch: "main".to_owned(),
  };
  note_change(&mut world, moved);
  assert_eq!(world.issues["k"].pending.as_ref().map(|p| p.action), Some(Action::Merge));
  let mut other = self::world();
  other.paused.insert("e".to_owned());
  let moved = Change::BaseMoved {
    repo: "o/r".to_owned(),
    branch: "main".to_owned(),
  };
  note_change(&mut other, moved);
  assert!(other.issues["k"].pending.is_none(), "a paused epic is not worked");
}
