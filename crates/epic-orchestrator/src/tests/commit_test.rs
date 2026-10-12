use super::commit_effect;
use crate::logic::ensure_issue;
use crate::model::{Action, Seen, Step, World, fresh};
use crate::watch::{Kind, Watch};

#[test]
fn an_applied_merge_moves_to_cleaning() {
  let mut world = World::new(0);
  {
    let issue = ensure_issue(&mut world, "a", "one", "/tmp");
    issue.pending = Some(fresh(Action::Merge, "t1".to_owned()));
    "running".clone_into(&mut issue.stage);
  }
  let _steps = commit_effect(&mut world, "a", "applied");
  assert_eq!(world.issues.get("a").expect("a").stage, "cleaning");
}

#[test]
fn an_applied_merge_requests_related_checks_without_moving_the_clock() {
  let mut world = World::new(0);
  let issue = ensure_issue(&mut world, "a", "one", "/tmp");
  issue.repo = "o/r".into();
  issue.pending = Some(fresh(Action::Merge, "t1".into()));
  for (id, kind) in [
    (
      "pr",
      Kind::Pr {
        key: "a".into(),
        repo: "o/r".into(),
        number: 1,
      },
    ),
    (
      "base",
      Kind::Base {
        repo: "o/r".into(),
        branch: "main".into(),
      },
    ),
    (
      "epic",
      Kind::Epic {
        key: "one".into(),
        repo: "o/r".into(),
        number: 402,
      },
    ),
    (
      "other",
      Kind::Pr {
        key: "b".into(),
        repo: "other/repo".into(),
        number: 2,
      },
    ),
  ] {
    let mut watch = Watch::new(kind, 0);
    assert!(watch.take_due(0).is_some());
    world.watches.insert(id.into(), watch);
  }
  let _steps = commit_effect(&mut world, "a", "applied");
  for id in ["pr", "base", "epic"] {
    assert!(world.watches[id].immediate);
    assert_eq!(world.watches[id].next_at_ms, 180_000);
  }
  assert!(!world.watches["other"].immediate);
}

#[test]
fn hold_deferred_and_cleanup_become_judgments() {
  let mut world = World::new(0);
  arm(&mut world, Action::Launch, false);
  let _steps = commit_effect(
    &mut world,
    "a",
    r#"{"outcome":"hold","with":"b","score":0.9,"source":"jev"}"#,
  );
  assert!(
    world
      .attention
      .iter()
      .any(|item| item.kind == "coupling-hold")
  );
  arm(&mut world, Action::Launch, false);
  let _steps = commit_effect(&mut world, "a", "deferred");
  assert_eq!(
    world.issues.get("a").expect("a").deferred.as_deref(),
    Some("launch")
  );
  arm(&mut world, Action::Cleanup, false);
  let _steps = commit_effect(&mut world, "a", "weird");
  assert!(
    world
      .attention
      .iter()
      .any(|item| item.kind == "cleanup-incomplete")
  );
  arm(&mut world, Action::Launch, false);
  let _steps = commit_effect(&mut world, "a", "applied");
  assert_eq!(world.issues.get("a").expect("a").stage, "running");
}

#[test]
fn a_recovered_effect_reconciles_or_defers() {
  let mut world = World::new(0);
  arm(&mut world, Action::Merge, true);
  let _steps = commit_effect(&mut world, "a", "deferred");
  assert!(world.issues.get("a").expect("a").pending.is_none());
  arm(&mut world, Action::Merge, true);
  world.watches.insert("epic".into(), epic_watch());
  let _steps = commit_effect(&mut world, "a", "merged");
  assert!(world.watches["epic"].immediate);
  let pending = world
    .issues
    .get("a")
    .expect("a")
    .pending
    .clone()
    .expect("pending");
  assert!(!pending.recovered);
  assert_eq!(pending.phase, 2);
  arm(&mut world, Action::Merge, true);
  let _steps = commit_effect(&mut world, "a", "open");
  assert!(
    !world
      .issues
      .get("a")
      .expect("a")
      .pending
      .as_ref()
      .expect("open")
      .recovered
  );
  arm(&mut world, Action::Merge, true);
  let _steps = commit_effect(&mut world, "a", "nope");
  assert!(world.attention.iter().any(|item| item.kind == "failed"));
  let _steps = commit_effect(&mut world, "missing", "applied");
}

fn epic_watch() -> Watch {
  let mut watch = Watch::new(
    Kind::Epic {
      key: "one".into(),
      repo: "o/r".into(),
      number: 402,
    },
    0,
  );
  assert!(watch.take_due(0).is_some());
  watch
}

#[test]
fn a_recovered_already_still_requests_an_immediate_check() {
  let mut world = World::new(0);
  arm(&mut world, Action::Merge, true);
  world.watches.insert("epic".into(), epic_watch());
  let _steps = commit_effect(&mut world, "a", "already");
  assert!(world.watches["epic"].immediate);
  assert_eq!(world.watches["epic"].next_at_ms, 180_000);
}

fn arm(world: &mut World, action: Action, recovered: bool) {
  let issue = ensure_issue(world, "a", "one", "/tmp");
  let mut pending = fresh(action, "t".to_owned());
  pending.recovered = recovered;
  pending.phase = 1;
  issue.pending = Some(pending);
  issue.stage = "running".to_owned();
}

#[test]
fn a_refused_cleanup_leaves_cleanup_incomplete_with_its_reason() {
  let mut world = World::new(0);
  arm(&mut world, Action::Cleanup, false);
  let doc = r#"{"outcome":"cleanup-incomplete","note":"owned workload still runs"}"#;
  let steps = commit_effect(&mut world, "a", doc);
  let issue = &world.issues["a"];
  assert_eq!(issue.stage, "cleanup-incomplete");
  assert!(issue.pending.is_none());
  assert!(
    steps
      .iter()
      .any(|step| matches!(step, Step::Issue { key } if key == "a"))
  );
  assert_eq!(world.attention[0].kind, "cleanup-incomplete");
  assert_eq!(world.attention[0].note, "owned workload still runs");
}

#[test]
fn a_finished_cleanup_journals_its_settlement_and_notes_the_closed_item() {
  let mut world = World::new(0);
  arm(&mut world, Action::Cleanup, false);
  let doc = r#"{"outcome":"applied","settle":{"issue":"closed"},"tracker_closed":true}"#;
  let steps = commit_effect(&mut world, "a", doc);
  assert!(steps.iter().any(|step| matches!(step, Step::Journal(row) if row.name == "settle" && row.note == r#"{"issue":"closed"}"#)));
  let issue = &world.issues["a"];
  assert_eq!(issue.tracker_seen, Seen::Closed);
  assert_eq!(issue.pending.as_ref().map(|pending| pending.phase), Some(2));
  assert_eq!(world.attention.len(), 0);
  arm(&mut world, Action::Cleanup, false);
  let doc = r#"{"outcome":"applied","settle_error":"JIRA_PAT is not set"}"#;
  let _steps = commit_effect(&mut world, "a", doc);
  assert_eq!(world.attention[0].kind, "failed");
  assert_eq!(world.attention[0].note, "settle: JIRA_PAT is not set");
}

#[test]
fn a_deferred_merge_raises_merge_ready() {
  let mut world = World::new(0);
  arm(&mut world, Action::Merge, false);
  let issue = world.issues.get_mut("a").expect("a");
  issue.repo = "o/r".into();
  issue.pr = Some(7);
  let _steps = commit_effect(&mut world, "a", "deferred");
  assert_eq!(world.issues["a"].deferred.as_deref(), Some("merge"));
  assert_eq!(world.attention[0].kind, "merge-ready");
  assert_eq!(
    world.attention[0].note,
    "run toolu epic gate --pr o/r#7 --merge"
  );
}
