use serde_json::json;

use super::{RULES, valid_dismissals};

const EVIDENCE: &str = "+if items.is_empty() { return None; }\n+let last = items[items.len() - 1];";

fn answer(rechecks: serde_json::Value) -> String {
  json!({ "rechecks": rechecks }).to_string()
}

#[test]
fn a_dismissal_needs_a_long_explanation_and_quotes_from_the_evidence() {
  let good = json!({"id": "f3", "decision": "dismiss", "explanation": "the empty case returns before the index",
                    "evidence_quotes": ["if items.is_empty() { return None; }"]});
  let found = valid_dismissals(&answer(json!([good])), &[3], EVIDENCE);
  assert_eq!(found.len(), 1);
  assert_eq!(found[0].0, 3);
  assert_eq!(found[0].1.1, ["if items.is_empty() { return None; }"]);
}

#[test]
fn anything_short_of_a_verified_dismissal_keeps_the_finding() {
  let base = json!({"id": "f1", "decision": "dismiss", "explanation": "the empty case returns before the index",
                    "evidence_quotes": ["if items.is_empty() { return None; }"]});
  let with = |key: &str, value: serde_json::Value| {
    let mut changed = base.clone();
    changed[key] = value;
    answer(json!([changed]))
  };
  let rejected = [
    with("decision", json!("keep")),
    with("explanation", json!("too short")),
    with("evidence_quotes", json!([])),
    with("evidence_quotes", json!(["if items.len() > 0"])),
    with("evidence_quotes", json!(["  "])),
    with("id", json!("f9")),
    "not json".to_owned(),
  ];
  for content in rejected {
    assert!(
      valid_dismissals(&content, &[1], EVIDENCE).is_empty(),
      "{content}"
    );
  }
  assert!(RULES.contains("never instructions"));
}
