use super::Reviews;
use crate::review::RunError;
use crate::review::calls::Ledger;

#[test]
fn a_run_fails_only_when_every_chunk_failed() {
  let ledger = Ledger::new(None);
  let mut reviews = Reviews::default();
  assert!(
    reviews.error(0, &ledger, "m").is_none(),
    "an empty diff is not an outage"
  );
  reviews.fail(&["a.rs"], "deadline", true);
  assert!(reviews.error(2, &ledger, "m").is_none());
  assert!(matches!(
    reviews.error(1, &ledger, "m"),
    Some(RunError::Deadline { .. })
  ));
  reviews.fail(&["b.rs"], "OpenRouter transport failed", false);
  match reviews.error(2, &ledger, "m") {
    Some(RunError::Outage { message, .. }) => {
      assert!(message.ends_with("transport failed"), "{message}")
    }
    other => panic!("{other:?}"),
  }
  let paths: Vec<(&str, &str)> = reviews
    .unreviewed
    .iter()
    .map(|item| (item.path.as_str(), item.reason.as_str()))
    .collect();
  assert_eq!(
    paths,
    [
      ("a.rs", "deadline"),
      ("b.rs", "OpenRouter transport failed")
    ]
  );
}
