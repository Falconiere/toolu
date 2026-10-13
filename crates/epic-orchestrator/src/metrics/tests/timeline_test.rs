use super::{Timeline, phase_of};
use crate::journal::Record;

fn fold(rows: &[(&str, &str, &str, u64)], now: u64) -> super::Times {
  let mut timeline = Timeline::default();
  for (kind, name, note, at) in rows {
    timeline.push("i1", &Record::new(kind, name, "i1", "", note), *at);
  }
  timeline.finish(now)
}

#[test]
fn lead_runs_from_the_first_launch_to_a_clean_cleanup_and_phases_split_it() {
  let times = fold(
    &[
      ("action", "launch-intent", "", 0),
      ("action", "launch-done", "wait", 5),
      ("action", "launch-intent", "", 10),
      ("action", "launch-done", "", 20),
      ("transition", "phase", "execution", 20),
      ("transition", "phase", "ready", 100),
      ("transition", "queue-left", "rebasing: conflict", 130),
      ("transition", "github-babysit-ready", "", 160),
      ("action", "cleanup-done", "failed: busy", 170),
      ("action", "cleanup-done", "", 200),
      ("transition", "phase", "babysit", 250),
    ],
    1_000,
  );
  assert_eq!(times.lead, [200]);
  assert_eq!(times.issues, 1);
  assert_eq!(times.relaunches, 0);
  let phases: Vec<(&str, u64)> = times.phases.iter().map(|(k, v)| (k.as_str(), *v)).collect();
  assert_eq!(phases, [("execution", 80), ("ready", 70), ("rebasing", 30)]);
}

#[test]
fn an_open_issue_counts_its_phase_up_to_now_and_relaunches_count_extra_launches() {
  let times = fold(
    &[
      ("action", "launch-intent", "", 0),
      ("action", "launch-done", "", 1),
      ("transition", "phase", "execution", 10),
      ("action", "launch-done", "", 50),
    ],
    110,
  );
  assert_eq!(times.lead, Vec::<u64>::new());
  assert_eq!(times.relaunches, 1);
  assert_eq!(times.phases.get("execution"), Some(&100));
}

#[test]
fn only_phase_changing_transitions_have_a_phase() {
  assert_eq!(phase_of("phase", "spec").as_deref(), Some("spec"));
  assert_eq!(phase_of("phase", ""), None);
  assert_eq!(
    phase_of("queue-left", "babysit").as_deref(),
    Some("babysit")
  );
  assert_eq!(phase_of("github-check", "{}"), None);
}
