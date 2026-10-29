use super::findings;
use crate::Verdict;

#[test]
fn findings_decide_the_verdict() {
  assert_eq!(findings::<String>("task", &[]), Verdict::Clean);
  assert_eq!(findings("task", &["one".to_owned()]), Verdict::Findings);
}
