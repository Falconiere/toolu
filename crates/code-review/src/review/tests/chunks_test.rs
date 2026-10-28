use super::Reviews;
use crate::review::RunError;
use crate::review::calls::{Call, Ledger, Stage};

fn sent() -> Call {
  Call {
    stage: Stage::Review,
    model: "m".into(),
    provider: None,
    tokens_in: 1,
    tokens_out: 1,
    cost_usd: None,
    millis: 1,
    outcome: "failed".into(),
  }
}

#[test]
fn a_run_fails_only_when_every_chunk_failed() {
  let mut ledger = Ledger::new(None);
  ledger.calls.push(sent());
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
      assert!(message.ends_with("transport failed"), "{message}");
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

#[test]
fn chunks_that_never_fit_the_budget_say_nothing_was_sent() {
  let ledger = Ledger::new(None);
  let mut reviews = Reviews::default();
  reviews.fail(&["a.rs"], "prompt over budget", false);
  match reviews.error(1, &ledger, "m") {
    Some(RunError::Outage { message, .. }) => {
      assert_eq!(
        message,
        "no chunk fit the prompt budget, so no review call was sent"
      );
    }
    other => panic!("{other:?}"),
  }
}
