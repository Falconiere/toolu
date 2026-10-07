use serde_json::{Value, json};

use crate::reply::parse;
use crate::{Answer, Error, Question, Questions, Usage};

fn noul() -> Questions {
  Questions::single("q", Question::noul("Urgent?", None, None).expect("noul"))
}

fn envelope(answers: &Value) -> String {
  json!({ "model": "jev-1.13.0", "answers": answers, "usage": { "input_tokens": 3, "output_tokens": 2 } })
    .to_string()
}

#[test]
fn a_typed_noul_answer_is_read_and_the_body_kept() {
  let body = envelope(&json!({ "q": { "type": "noul", "noul": 0.92 } }));
  let reply = parse(&body, &noul()).expect("reply");
  assert_eq!(reply.model, "jev-1.13.0");
  assert_eq!(
    reply.usage,
    Usage {
      input_tokens: 3,
      output_tokens: 2
    }
  );
  assert_eq!(
    reply.answers,
    [("q".to_owned(), Answer::Noul { noul: 0.92 })]
  );
  assert_eq!(reply.body, body);
}

#[test]
fn integral_float_counts_pass_and_huge_ones_saturate() {
  let body = json!({ "model": "m", "answers": { "q": { "type": "noul", "noul": 0.5 } },
                     "usage": { "input_tokens": 3.0, "output_tokens": 1e30 } })
  .to_string();
  let usage = parse(&body, &noul()).expect("reply").usage;
  assert_eq!((usage.input_tokens, usage.output_tokens), (3, u64::MAX));
}

#[test]
fn a_malformed_envelope_is_no_judgment() {
  let answers = json!({ "q": { "type": "noul", "noul": 0.5 } });
  let bodies = [
    "not json".to_owned(),
    json!({ "model": "x" }).to_string(),
    json!({ "model": "", "answers": answers, "usage": { "input_tokens": 1, "output_tokens": 1 } }).to_string(),
    json!({ "model": "m", "answers": answers, "usage": { "input_tokens": -1, "output_tokens": 2 } }).to_string(),
    json!({ "model": "m", "answers": answers, "usage": { "input_tokens": 1.5, "output_tokens": 2 } }).to_string(),
    envelope(&json!({})),
    envelope(&json!({ "q": { "type": "noul", "noul": 1.2 } })),
    envelope(&json!({ "q": { "type": "choice", "choice": "a" } })),
    envelope(&json!({ "q": { "type": "noul", "noul": 0.5 }, "extra": {} })),
  ];
  for body in bodies {
    assert_eq!(parse(&body, &noul()), Err(Error::InvalidResponse), "{body}");
  }
}

#[test]
fn choice_and_score_answers_must_match_their_criteria() {
  let choice = Questions::single(
    "q",
    Question::choice("Which?", &[("a", None), ("b", None)]).expect("choice"),
  );
  let good = json!({ "q": { "type": "choice", "choice": "a", "probabilities": { "a": 0.8, "b": 0.2 }, "confidence": 0.8 } });
  assert!(parse(&envelope(&good), &choice).is_ok());
  for bad in [
    json!({ "q": { "type": "choice", "choice": "a", "probabilities": { "a": 0.8, "b": 0.1 }, "confidence": 0.8 } }),
    json!({ "q": { "type": "choice", "choice": "c", "probabilities": { "a": 0.8, "b": 0.2 }, "confidence": 0.8 } }),
    json!({ "q": { "type": "choice", "choice": "a", "probabilities": { "a": 1.0 }, "confidence": 0.8 } }),
  ] {
    assert_eq!(parse(&envelope(&bad), &choice), Err(Error::InvalidResponse));
  }
  let score = Questions::single(
    "q",
    Question::score("How?", &["Calm", "Angry", "Furious"]).expect("score"),
  );
  let good = json!({ "q": { "type": "score", "score": 1.5,
    "legend": { "0": "Calm", "1": "Angry", "2": "Furious" },
    "probabilities": { "0": 0.1, "1": 0.3, "2": 0.6 }, "confidence": 0.8 } });
  let reply = parse(&envelope(&good), &score).expect("score");
  assert!(
    matches!(reply.answers[0].1, Answer::Score { score, .. } if (score - 1.5).abs() < f64::EPSILON)
  );
  let mut bad = good.clone();
  bad["q"]["score"] = json!(2.5);
  assert_eq!(parse(&envelope(&bad), &score), Err(Error::InvalidResponse));
  let mut bad = good;
  bad["q"]["legend"] = json!({ "0": "Calm" });
  assert_eq!(parse(&envelope(&bad), &score), Err(Error::InvalidResponse));
}

#[test]
fn an_ask_question_of_unknown_type_or_shape_gets_no_judgment() {
  let unknown = Questions::parse(r#"{"q":{"type":"bogus","instructions":"?"}}"#).expect("payload");
  let reply = envelope(&json!({ "q": { "type": "bogus" } }));
  assert_eq!(parse(&reply, &unknown), Err(Error::InvalidResponse));
  let shapeless = Questions::parse(r#"{"q":{"type":"choice","instructions":"?","criteria":"x"}}"#)
    .expect("payload");
  let reply = envelope(
    &json!({ "q": { "type": "choice", "choice": "x", "probabilities": { "x": 1 }, "confidence": 1 } }),
  );
  assert_eq!(parse(&reply, &shapeless), Err(Error::InvalidResponse));
}
