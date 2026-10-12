use serde_json::{Value, json};
use toolu_http_test_support::{Fixture, Reply};

use super::jev_scores;
use crate::route::tier::Evidence;
use crate::support::jev;

fn answer(score: f64) -> Value {
  json!({"type": "score", "score": score, "legend": {"0": "a", "1": "b", "2": "c", "3": "d"},
    "probabilities": {"0": 0.1, "1": 0.2, "2": 0.3, "3": 0.4}, "confidence": 0.7})
}

#[test]
fn jev_scores_each_issue_from_its_evidence() {
  let fixture = Fixture::start().expect("fixture");
  let body = json!({"model": "m", "usage": {"input_tokens": 1, "output_tokens": 1},
    "answers": {"a-1": answer(1.36), "a-2": answer(0.2)}});
  fixture
    .route("/v1/systemone", Reply::new(200, body.to_string()))
    .expect("route");
  let issues = vec![
    Evidence {
      key: "a-1".into(),
      title: "Big".into(),
      open_blockers: 2,
      unblocks: 3,
      ..Evidence::default()
    },
    Evidence {
      key: "a-2".into(),
      title: "Small".into(),
      ..Evidence::default()
    },
  ];
  let scores = jev_scores(&jev(&fixture), &issues).expect("scores");
  assert_eq!(scores["a-1"].tier, 2);
  assert_eq!(scores["a-2"].tier, 0);
  assert_eq!(scores["a-1"].source, "jev");
  let sent: Value =
    serde_json::from_slice(&fixture.requests().expect("requests")[0].body).expect("json");
  assert_eq!(sent["state"]["issues"]["a-1"]["open_blockers"], 2);
  assert_eq!(sent["questions"]["a-2"]["type"], "score");
}

#[test]
fn a_jev_failure_is_reported() {
  let fixture = Fixture::start().expect("fixture");
  fixture
    .route("/v1/systemone", Reply::new(400, "{}"))
    .expect("route");
  let issues = vec![Evidence {
    key: "a-1".into(),
    ..Evidence::default()
  }];
  let err = jev_scores(&jev(&fixture), &issues).expect_err("refused");
  assert!(err.starts_with("jev failed:"), "{err}");
}
