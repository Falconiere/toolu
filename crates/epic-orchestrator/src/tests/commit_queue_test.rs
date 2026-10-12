use serde_json::json;

use super::{commit, leave};
use crate::model::{Action, Issue, Step, World, fresh};
use crate::queue_fields::Rounds;
use crate::watch::{Kind, Watch};

/// Two ready issues of `o/r` (`a` ahead of `b`), `a` running its step.
fn world() -> World {
  let mut world = World::new(1_000);
  for (key, at) in [("a", 10), ("b", 20)] {
    let mut issue = Issue::blank(key, "e", "/state", 0);
    "ready".clone_into(&mut issue.phase);
    "running".clone_into(&mut issue.stage);
    "o/r".clone_into(&mut issue.repo);
    issue.pr = Some(if key == "a" { 1 } else { 2 });
    issue.queue.queued_at_ms = Some(at);
    world.issues.insert(key.to_owned(), issue);
  }
  if let Some(issue) = world.issues.get_mut("a") {
    issue.pending = Some(fresh(Action::Merge, "t1".to_owned()));
  }
  let kind = Kind::Pr {
    key: "a".to_owned(),
    repo: "o/r".to_owned(),
    number: 1,
  };
  let mut watch = Watch::new(kind, 0);
  watch.immediate = false;
  world.watches.insert("pr:o/r#1".to_owned(), watch);
  world
}

fn names(steps: &[Step]) -> Vec<String> {
  steps
    .iter()
    .filter_map(|step| match step {
      Step::Journal(record) => Some(format!("{}:{}", record.name, record.note)),
      Step::Prompt { text, .. } => Some(format!("prompt:{text}")),
      Step::Call { .. } | Step::Status { .. } | Step::Issue { .. } | Step::Notify { .. } => None,
    })
    .collect()
}

#[test]
fn an_engine_rebase_keeps_the_head_counts_the_round_and_checks_ci_now() {
  let mut world = world();
  let doc = json!({"outcome": "rebased", "head": "old", "new_head": "new"}).to_string();
  let steps = commit(&mut world, "a", &doc).expect("queue doc");
  assert_eq!(
    names(&steps),
    [
      "merge-done:rebased",
      r#"rebase:{"by":"engine","head":"new","round":1}"#
    ]
  );
  let a = &world.issues["a"];
  assert_eq!((a.pending.is_none(), a.phase.as_str()), (true, "ready"));
  assert_eq!(
    a.queue.rebase_rounds,
    Rounds {
      engine: 1,
      worker: 0
    }
  );
  assert!(world.watches["pr:o/r#1"].immediate);
  assert!(
    world.issues["b"].pending.is_none(),
    "b waits without a rebase"
  );
}

#[test]
fn a_conflict_hands_off_once_leaves_the_queue_and_arms_the_next_head() {
  let mut world = world();
  let doc = json!({"outcome": "handoff", "note": "conflict: a.txt"}).to_string();
  let steps = commit(&mut world, "a", &doc).expect("queue doc");
  let names = names(&steps);
  assert_eq!(names[0], "merge-done:handoff: conflict: a.txt");
  assert_eq!(names[1], r#"rebase:{"by":"worker","round":1}"#);
  assert_eq!(names[2], "queue-left:rebasing: conflict: a.txt");
  assert_eq!(
    names
      .iter()
      .filter(|name| name.starts_with("prompt:"))
      .count(),
    1
  );
  assert!(names.contains(&"prompt:REBASE".to_owned()));
  let a = &world.issues["a"];
  assert_eq!((a.phase.as_str(), a.queue.queued_at_ms), ("rebasing", None));
  assert_eq!(
    world.issues["b"].pending.as_ref().map(|p| p.action),
    Some(Action::Merge)
  );
}

#[test]
fn a_fix_verdict_returns_to_babysit_and_disarms_what_the_engine_armed() {
  let mut world = world();
  let waiting = json!({"outcome": "waiting", "auto_merge": "armed"}).to_string();
  let _ = commit(&mut world, "a", &waiting).expect("waiting");
  assert!(world.issues["a"].queue.auto_armed);
  leave(&mut world, "a", "babysit", "new thread");
  let a = &world.issues["a"];
  assert_eq!(a.phase, "babysit");
  assert!(a.queue.disarm_due, "the armed auto-merge is due a disarm");
  let mut world = self::world();
  let fix = json!({"outcome": "fix", "reasons": ["failing: test"]}).to_string();
  let steps = commit(&mut world, "a", &fix).expect("fix");
  assert!(names(&steps).contains(&r#"queue-left:babysit: ["failing: test"]"#.to_owned()));
  assert!(
    world.issues["a"].pending.is_none(),
    "the step already disarmed"
  );
}

#[test]
fn waits_refusals_and_stale_merges_keep_the_head_and_never_prompt() {
  for (outcome, extra) in [
    ("waiting", json!({"auto_merge": "unavailable"})),
    ("busy", json!({"note": "worker active"})),
    ("stale", json!({"note": "Head branch was modified"})),
    ("error", json!({"note": "502"})),
    ("disarmed", json!({})),
  ] {
    let mut world = world();
    let mut doc = extra.clone();
    doc["outcome"] = json!(outcome);
    let steps = commit(&mut world, "a", &doc.to_string()).expect("doc");
    let names = names(&steps);
    assert!(
      names[0].starts_with(&format!("merge-done:{outcome}")),
      "{names:?}"
    );
    assert!(
      !names.iter().any(|name| name.starts_with("prompt:")),
      "{names:?}"
    );
    assert_eq!(world.issues["a"].phase, "ready");
    assert!(world.issues["b"].pending.is_none());
    assert!(!world.issues["a"].queue.auto_armed);
  }
  let mut world = world();
  let _ = commit(
    &mut world,
    "a",
    r#"{"outcome":"busy","note":"worker active"}"#,
  );
  assert!(world.outbox.is_empty());
}

#[test]
fn an_uncertain_merge_is_held_and_a_closed_pr_frees_the_queue() {
  let mut world = world();
  let steps = commit(
    &mut world,
    "a",
    r#"{"outcome":"uncertain","note":"timed out"}"#,
  )
  .expect("doc");
  assert_eq!(world.issues["a"].deferred.as_deref(), Some("merge"));
  assert_eq!(world.attention[0].kind, "failed");
  assert!(names(&steps)[0].starts_with("merge-done:uncertain"));
  let mut world = self::world();
  let _ = commit(&mut world, "a", r#"{"outcome":"closed"}"#).expect("doc");
  assert_eq!(world.issues["a"].stage, "closed");
  assert_eq!(world.attention[0].kind, "pr-closed");
  assert_eq!(
    world.issues["b"].pending.as_ref().map(|p| p.action),
    Some(Action::Merge)
  );
}

#[test]
fn a_merged_head_moves_to_cleaning_and_plain_outcomes_are_not_queue_docs() {
  let mut world = world();
  let _ = commit(
    &mut world,
    "a",
    r#"{"outcome":"merged","admin_used":false}"#,
  )
  .expect("doc");
  let a = &world.issues["a"];
  assert_eq!(
    (a.stage.as_str(), a.pending.as_ref().map(|p| p.phase)),
    ("cleaning", Some(2))
  );
  assert_eq!(commit(&mut world, "a", "applied"), None);
  assert_eq!(commit(&mut world, "a", r#"{"note":"x"}"#), None);
}

#[test]
fn a_late_armed_wait_on_an_issue_that_left_is_due_a_disarm() {
  let mut world = world();
  "babysit".clone_into(&mut world.issues.get_mut("a").expect("a").phase);
  let waiting = json!({"outcome": "waiting", "auto_merge": "armed"}).to_string();
  let _ = commit(&mut world, "a", &waiting).expect("waiting");
  assert!(world.issues["a"].queue.disarm_due);
  let mut world = self::world();
  world.issues.get_mut("a").expect("a").queue.disarm_due = true;
  "babysit".clone_into(&mut world.issues.get_mut("a").expect("a").phase);
  let _ = commit(&mut world, "a", r#"{"outcome":"error","note":"502"}"#).expect("error");
  assert_eq!(
    world.issues["a"].queue.disarm_errors, 1,
    "a failed disarm is retried"
  );
}

#[test]
fn a_held_babysit_prompt_goes_out_once_the_step_commits() {
  let mut world = world();
  world.issues.get_mut("a").expect("a").queue.held = Some(crate::queue_fields::Held {
    prompt: "fix the thread".to_owned(),
    note: Some("escalated".to_owned()),
  });
  let steps = commit(
    &mut world,
    "a",
    r#"{"outcome":"busy","note":"worker active"}"#,
  )
  .expect("doc");
  let sent = names(&steps);
  assert!(
    sent.contains(&"prompt:fix the thread".to_owned()),
    "{sent:?}"
  );
  assert!(
    sent
      .iter()
      .any(|name| name.starts_with("queue-left:babysit"))
  );
  assert_eq!(world.attention[0].kind, "babysit");
  assert!(world.issues["a"].queue.held.is_none());
  let mut world = self::world();
  world.issues.get_mut("a").expect("a").queue.held = Some(crate::queue_fields::Held {
    prompt: "fix the thread".to_owned(),
    note: None,
  });
  let steps = commit(
    &mut world,
    "a",
    r#"{"outcome":"handoff","note":"conflict"}"#,
  )
  .expect("doc");
  let prompts: Vec<String> = names(&steps)
    .into_iter()
    .filter(|n| n.starts_with("prompt:"))
    .collect();
  assert_eq!(prompts, ["prompt:REBASE"], "only the rebase prompt");
}
