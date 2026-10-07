use crate::{Error, Question, Questions, State};

fn text(questions: &Questions) -> String {
  questions.value().to_text(false)
}

#[test]
fn state_is_text_unless_it_is_a_json_object_or_array() {
  assert_eq!(
    State::text("Payouts failing").value().to_text(false),
    r#""Payouts failing""#
  );
  assert_eq!(
    State::structured("{\"ticket\":\"Payouts failing\"}\n")
      .value()
      .to_text(false),
    r#"{"ticket":"Payouts failing"}"#
  );
  assert_eq!(State::structured("[1,2]").value().to_text(false), "[1,2]");
  assert_eq!(State::structured("123\n"), State::text("123"));
  assert_eq!(
    State::structured("Payouts failing\n\n"),
    State::text("Payouts failing")
  );
  assert_eq!(State::structured("[1]\n").value().to_text(false), "[1]");
  for scalar in ["123", "true", "null", "\"quoted\"", "not json"] {
    assert_eq!(State::structured(scalar), State::text(scalar), "{scalar}");
  }
}

#[test]
fn noul_sends_only_the_sides_it_was_given() {
  let bare = Questions::single("q", Question::noul("Urgent?", None, None).expect("noul"));
  assert_eq!(
    text(&bare),
    r#"{"q":{"type":"noul","instructions":"Urgent?"}}"#
  );
  let sides = Question::noul("Urgent?", Some("yes it is"), Some("")).expect("noul");
  assert_eq!(
    text(&Questions::single("q", sides)),
    r#"{"q":{"type":"noul","instructions":"Urgent?","criteria":{"true":"yes it is"}}}"#
  );
}

#[test]
fn choice_keeps_option_order_and_a_repeated_key_keeps_its_first_place() {
  let options = [
    ("billing", Some("Payments")),
    ("tech", None),
    ("billing", Some("Money")),
  ];
  let choice = Question::choice("Which team?", &options).expect("choice");
  assert_eq!(
    text(&Questions::single("q", choice)),
    r#"{"q":{"type":"choice","instructions":"Which team?","criteria":{"billing":"Money","tech":null}}}"#
  );
  let proto = Question::choice(
    "Which?",
    &[("__proto__", Some("prototype")), ("other", None)],
  );
  assert_eq!(
    text(&Questions::single("q", proto.expect("choice"))),
    r#"{"q":{"type":"choice","instructions":"Which?","criteria":{"__proto__":"prototype","other":null}}}"#
  );
}

#[test]
fn score_sends_its_levels_lowest_first() {
  let score = Question::score("Severity?", &["Calm", "Angry", "Furious"]).expect("score");
  assert_eq!(
    text(&Questions::single("q", score)),
    r#"{"q":{"type":"score","instructions":"Severity?","criteria":["Calm","Angry","Furious"]}}"#
  );
}

#[test]
fn builders_refuse_bad_shapes_before_any_request() {
  let many: Vec<String> = (0..256).map(|index| format!("key{index}")).collect();
  let many: Vec<(&str, Option<&str>)> = many.iter().map(|key| (key.as_str(), None)).collect();
  let levels: Vec<String> = (0..11).map(|index| index.to_string()).collect();
  let levels: Vec<&str> = levels.iter().map(String::as_str).collect();
  let cases = [
    (
      Question::choice("Which?", &[("one", None)]),
      "choice needs at least 2 options",
    ),
    (
      Question::choice("Which?", &many),
      "choice accepts at most 255 options",
    ),
    (
      Question::choice("Which?", &[("", None), ("b", None)]),
      "a choice option needs a key",
    ),
    (
      Question::score("How?", &["low"]),
      "score needs at least 2 levels",
    ),
    (
      Question::score("How?", &levels),
      "score accepts at most 10 levels",
    ),
    (Question::noul("", None, None), "noul needs instructions"),
    (
      Question::choice("", &[("a", None), ("b", None)]),
      "choice needs instructions",
    ),
    (
      Question::score("", &["low", "high"]),
      "score needs instructions",
    ),
  ];
  for (result, message) in cases {
    assert_eq!(result, Err(Error::InvalidQuestion(message.to_owned())));
  }
}

#[test]
fn an_ask_payload_is_a_non_empty_object_sent_as_given() {
  let payload =
    r#"{"urgent":{"type":"noul","instructions":"Urgent?","weight":3},"b":{"type":"x"}}"#;
  assert_eq!(text(&Questions::parse(payload).expect("payload")), payload);
  let cases = [
    ("{}", "questions must not be empty"),
    ("[]", "questions must be a JSON object"),
    ("not json", "questions are not valid JSON"),
  ];
  for (payload, message) in cases {
    assert_eq!(
      Questions::parse(payload),
      Err(Error::InvalidQuestion(message.to_owned()))
    );
  }
}
