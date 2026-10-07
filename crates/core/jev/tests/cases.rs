//! The request/response cases of `plugins/jev/hooks/src/__tests__/jev.test.ts`
//! that do not concern argv, against the loopback endpoint.

#[path = "helpers/endpoint.rs"]
mod endpoint;

use endpoint::{Endpoint, PATH};
use serde_json::{Value, json};
use toolu_http_test_support::Reply;
use toolu_jev_client::{Answer, DEFAULT_MODEL, Error, Question, Questions, State};

fn answer() -> Value {
  json!({ "model": "jev-1.13.0", "answers": { "q": { "type": "noul", "noul": 0.92 } },
          "usage": { "input_tokens": 3, "output_tokens": 2 } })
}

fn noul() -> Result<Questions, Error> {
  Ok(Questions::single(
    "q",
    Question::noul("Urgent?", None, None)?,
  ))
}

#[test]
fn noul_sends_one_typed_question_and_reads_its_answer() {
  let endpoint = Endpoint::start().expect("endpoint");
  endpoint
    .fixture
    .route(PATH, Reply::new(200, answer().to_string()))
    .expect("route");
  let jev = endpoint.jev(&[]).expect("jev");
  let reply = jev
    .ask(
      &State::text("Payouts failing for 3 days"),
      DEFAULT_MODEL,
      &noul().expect("noul"),
    )
    .expect("reply");
  assert_eq!(
    reply.answers,
    [("q".to_owned(), Answer::Noul { noul: 0.92 })]
  );
  let requests = endpoint.fixture.requests().expect("requests");
  assert_eq!(
    (requests[0].method.as_str(), requests[0].path.as_str()),
    ("POST", PATH)
  );
  assert_eq!(
    requests[0].headers.get("authorization").map(String::as_str),
    Some("Bearer fixture-key")
  );
  assert_eq!(
    endpoint.bodies(),
    [
      r#"{"state":"Payouts failing for 3 days","model":"jev-latest","questions":{"q":{"type":"noul","instructions":"Urgent?"}}}"#
    ]
  );
  assert!(endpoint.fixture.connects().expect("connects")[0].starts_with("api.example.test:"));
}

#[test]
fn a_missing_or_broken_key_fails_before_any_request() {
  let endpoint = Endpoint::start().expect("endpoint");
  assert_eq!(
    endpoint.jev(&[("TYPESAFE_API_KEY", "")]).map(|_| ()),
    Err(Error::MissingKey)
  );
  assert_eq!(
    endpoint
      .jev(&[("TYPESAFE_API_KEY", "bad\nkey")])
      .map(|_| ()),
    Err(Error::KeyLineBreak)
  );
  assert_eq!(endpoint.fixture.requests().expect("requests").len(), 0);
}

#[test]
fn choice_and_score_keep_their_criteria_shapes() {
  let endpoint = Endpoint::start().expect("endpoint");
  let jev = endpoint.jev(&[]).expect("jev");
  let choice = json!({ "model": "m", "usage": { "input_tokens": 3, "output_tokens": 2 },
    "answers": { "q": { "type": "choice", "choice": "billing",
      "probabilities": { "billing": 0.8, "tech": 0.2 }, "confidence": 0.8 } } });
  endpoint
    .fixture
    .route(PATH, Reply::new(200, choice.to_string()))
    .expect("route");
  let question = Question::choice(
    "Which team?",
    &[("billing", Some("Payments")), ("tech", None)],
  );
  jev
    .ask(
      &State::text("ticket"),
      DEFAULT_MODEL,
      &Questions::single("q", question.expect("choice")),
    )
    .expect("choice");
  assert_eq!(
    endpoint.last()["questions"]["q"]["criteria"],
    json!({ "billing": "Payments", "tech": null })
  );
  let score = json!({ "model": "m", "usage": { "input_tokens": 3, "output_tokens": 2 },
    "answers": { "q": { "type": "score", "score": 1.5, "legend": { "0": "Calm", "1": "Angry", "2": "Furious" },
      "probabilities": { "0": 0.1, "1": 0.3, "2": 0.6 }, "confidence": 0.8 } } });
  endpoint
    .fixture
    .route(PATH, Reply::new(200, score.to_string()))
    .expect("route");
  let question = Question::score("Severity?", &["Calm", "Angry", "Furious"]).expect("score");
  jev
    .ask(
      &State::text("ticket"),
      DEFAULT_MODEL,
      &Questions::single("q", question),
    )
    .expect("score");
  assert_eq!(
    endpoint.last()["questions"]["q"]["criteria"],
    json!(["Calm", "Angry", "Furious"])
  );
}

#[test]
fn ask_sends_its_payload_and_keeps_the_raw_reply() {
  let endpoint = Endpoint::start().expect("endpoint");
  let jev = endpoint.jev(&[]).expect("jev");
  let raw =
    json!({ "model": "jev-1.13.0", "answers": { "urgent": { "type": "noul", "noul": 0.92 } },
                    "usage": { "input_tokens": 3, "output_tokens": 2 } })
    .to_string();
  endpoint
    .fixture
    .route(PATH, Reply::new(200, raw.clone()))
    .expect("route");
  let ask =
    Questions::parse(r#"{"urgent":{"type":"noul","instructions":"Urgent?"}}"#).expect("ask");
  let reply = jev
    .ask(&State::text("ticket"), DEFAULT_MODEL, &ask)
    .expect("ask");
  assert_eq!(reply.body, raw);
  assert_eq!(
    endpoint.last()["questions"],
    json!({ "urgent": { "type": "noul", "instructions": "Urgent?" } })
  );
}

#[test]
fn a_choice_option_named_proto_stays_a_data_key_in_its_place() {
  let endpoint = Endpoint::start().expect("endpoint");
  let reply = json!({ "model": "m", "usage": { "input_tokens": 3, "output_tokens": 2 },
    "answers": { "q": { "type": "choice", "choice": "__proto__",
      "probabilities": { "__proto__": 0.6, "other": 0.4 }, "confidence": 0.6 } } });
  endpoint
    .fixture
    .route(PATH, Reply::new(200, reply.to_string()))
    .expect("route");
  let question = Question::choice(
    "Which?",
    &[("__proto__", Some("prototype")), ("other", None)],
  );
  endpoint
    .jev(&[])
    .expect("jev")
    .ask(
      &State::text("ticket"),
      DEFAULT_MODEL,
      &Questions::single("q", question.expect("choice")),
    )
    .expect("reply");
  assert!(endpoint.bodies()[0].contains(r#""criteria":{"__proto__":"prototype","other":null}"#));
}

#[test]
fn ask_pins_the_model_and_state_keeps_its_shape() {
  let endpoint = Endpoint::start().expect("endpoint");
  let urgent = json!({ "model": "m", "usage": { "input_tokens": 3, "output_tokens": 2 },
    "answers": { "urgent": { "type": "noul", "noul": 0.92 } } });
  endpoint
    .fixture
    .route(PATH, Reply::new(200, urgent.to_string()))
    .expect("route");
  let jev = endpoint.jev(&[]).expect("jev");
  let ask =
    Questions::parse(r#"{"urgent":{"type":"noul","instructions":"Urgent?"}}"#).expect("ask");
  jev
    .ask(&State::text("ticket"), "jev-1.13", &ask)
    .expect("ask");
  assert_eq!(endpoint.last()["model"], "jev-1.13");
  jev
    .ask(
      &State::structured("{\"ticket\":\"Payouts failing\"}"),
      "m",
      &ask,
    )
    .expect("structured");
  assert_eq!(
    endpoint.last()["state"],
    json!({ "ticket": "Payouts failing" })
  );
  jev
    .ask(&State::structured("123"), "m", &ask)
    .expect("scalar");
  assert_eq!(endpoint.last()["state"], "123");
}

#[test]
fn malformed_success_cannot_become_a_judgment() {
  let endpoint = Endpoint::start().expect("endpoint");
  let jev = endpoint.jev(&[]).expect("jev");
  let mut negative = answer();
  negative["usage"]["input_tokens"] = json!(-1);
  let bodies = [
    "not json".to_owned(),
    r#"{"model":"x"}"#.to_owned(),
    json!({ "model": "m", "answers": { "q": { "type": "noul", "noul": 1.2 } },
            "usage": { "input_tokens": 3, "output_tokens": 2 } })
    .to_string(),
    json!({ "model": "m", "answers": {}, "usage": { "input_tokens": 3, "output_tokens": 2 } })
      .to_string(),
    json!({ "model": "m", "answers": { "q": { "type": "choice", "choice": "a" } },
            "usage": { "input_tokens": 3, "output_tokens": 2 } })
    .to_string(),
    negative.to_string(),
  ];
  for body in bodies {
    endpoint
      .fixture
      .route(PATH, Reply::new(200, body.clone()))
      .expect("route");
    assert_eq!(
      jev.ask(&State::text("x"), DEFAULT_MODEL, &noul().expect("noul")),
      Err(Error::InvalidResponse),
      "{body}"
    );
  }
}

#[test]
fn a_terminal_http_error_returns_its_body_without_the_key() {
  let endpoint = Endpoint::start().expect("endpoint");
  endpoint
    .fixture
    .route(PATH, Reply::new(401, r#"{"error":"bad key fixture-key"}"#))
    .expect("route");
  let result =
    endpoint
      .jev(&[])
      .expect("jev")
      .ask(&State::text("x"), DEFAULT_MODEL, &noul().expect("noul"));
  assert_eq!(
    result,
    Err(Error::Http {
      status: 401,
      body: r#"{"error":"bad key <redacted>"}"#.into()
    })
  );
  assert_eq!(endpoint.fixture.requests().expect("requests").len(), 1);
}
