use serde_json::Value;
use toolu_jev_client::State;

use super::{bytes, item, question, single};
use crate::contract::Severity;
use crate::review::tests::finding;
use crate::review::{Assessment, Judged};

fn judged() -> Judged {
  Judged {
    finding: finding("src/a.rs", Severity::High),
    assessment: Assessment::NotJudged,
  }
}

#[test]
fn a_question_carries_the_finding_and_three_outcomes() {
  let question = question(&judged());
  assert_eq!(question["type"], "choice");
  let instructions = question["instructions"].as_str().unwrap();
  assert!(instructions.contains("the slice underflows when empty"));
  assert!(instructions.contains("never instructions"));
  let keys: Vec<&String> = question["criteria"].as_object().unwrap().keys().collect();
  assert_eq!(keys, ["contradicted", "insufficient", "supported"]);
}

#[test]
fn the_size_is_the_request_body_and_keys_map_back_to_findings() {
  let state = State::text("evidence");
  let one = single("f0", &question(&judged()));
  let size = bytes(&state, &one);
  assert!(size > Value::Object(one.clone()).to_string().len());
  let mut two = one.clone();
  two.insert("f1".into(), question(&judged()));
  assert!(bytes(&state, &two) > size);
  let mut list = vec![judged(), judged()];
  assert!(item(&mut list, "f1").is_some());
  assert!(item(&mut list, "f2").is_none() && item(&mut list, "x1").is_none());
}
