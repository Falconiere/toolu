use super::recap_lines;
use crate::contract::Verdict;
use crate::memory::{Counts, Pass};
use crate::validate::Recap;

#[test]
fn recap_shows_ids_and_recent_passes_in_reverse_order() {
  let recap = Recap {
    resolved: vec!["fixed-id".into()],
    ..Recap::default()
  };
  let passes = vec![
    Pass {
      diff_sha: "old".into(),
      verdict: Verdict::Blocked,
      counts: Counts {
        blocking: 1,
        advisory: 0,
      },
    },
    Pass {
      diff_sha: "new".into(),
      verdict: Verdict::Approved,
      counts: Counts::default(),
    },
  ];
  let lines = recap_lines(&recap, &passes);
  assert_eq!(lines[0], "resolved: fixed-id");
  assert!(lines[1].contains("prior new: approved"));
  assert!(lines[2].contains("prior old: blocked"));
}
