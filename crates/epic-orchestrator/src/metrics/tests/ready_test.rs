use super::Ready;
use crate::journal::Record;

fn split(rows: &[(&str, &str, &str, &str, u64)]) -> super::Split {
  let mut ready = Ready::default();
  for (issue, kind, name, note, at) in rows {
    ready.push(issue, &Record::new(kind, name, issue, "", note), *at);
  }
  ready.finish_all()
}

#[test]
fn a_clean_issue_splits_into_queue_rebase_ci_and_merge() {
  let split = split(&[
    ("i1", "transition", "phase", "ready", 0),
    ("i1", "action", "merge-intent", "", 10),
    ("i1", "action", "merge-done", "rebased", 20),
    ("i1", "action", "merge-intent", "", 50),
    ("i1", "action", "merge-done", "", 55),
    ("i1", "transition", "phase", "ready", 60),
  ]);
  assert_eq!(split.total, [55]);
  let buckets: Vec<u64> = split.buckets.iter().flatten().copied().collect();
  assert_eq!(buckets, [10, 10, 30, 5, 0]);
  assert_eq!(split.clean_excluding_ci, [25]);
}

#[test]
fn a_hand_off_is_rework_until_ready_and_the_detector_can_end_the_run() {
  let split = split(&[
    ("i2", "transition", "github-babysit-ready", "", 0),
    ("i2", "action", "merge-intent", "", 5),
    ("i2", "action", "merge-done", "handoff: conflict", 8),
    ("i2", "transition", "phase", "ready", 40),
    ("i2", "transition", "github-pr-merged", "", 50),
    ("i3", "transition", "phase", "ready", 0),
  ]);
  assert_eq!(split.total, [50]);
  let buckets: Vec<u64> = split.buckets.iter().flatten().copied().collect();
  assert_eq!(buckets, [18, 0, 0, 0, 32]);
  assert_eq!(split.clean_excluding_ci, Vec::<u64>::new());
}

#[test]
fn queue_left_counts_as_rework_and_a_step_without_a_run_is_ignored() {
  let split = split(&[
    ("i4", "action", "merge-done", "waiting", 1),
    ("i4", "transition", "phase", "ready", 10),
    (
      "i4",
      "transition",
      "queue-left",
      "babysit: failing check",
      20,
    ),
    ("i4", "transition", "phase", "ready", 70),
    ("i4", "action", "merge-intent", "", 75),
    ("i4", "action", "merge-done", "", 80),
  ]);
  assert_eq!(split.total, [70]);
  let buckets: Vec<u64> = split.buckets.iter().flatten().copied().collect();
  assert_eq!(buckets, [15, 0, 0, 5, 50]);
}
