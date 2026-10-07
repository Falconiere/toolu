use toolu_runtime::json::ordered::Ordered;

use super::{
  Advisories, Unreadable, final_advisory, final_ask, parse_document, printed, read_field,
  sanitize_surrogates, substituted,
};

fn doc(text: &str) -> Option<Ordered> {
  parse_document(text).ok()
}

#[test]
fn surrogate_escapes_are_kept_only_when_paired() {
  let cases = [
    (r#""\ud800""#, r#""\ufffd""#),
    (r#""\ud83d\ude00""#, r#""\ud83d\ude00""#),
    (r#""\ud800\u0041""#, r#""\ufffd\u0041""#),
    (r#""\udc00x""#, r#""\ufffdx""#),
    (r#""\\ud800""#, r#""\\ud800""#),
    (r#"\ud800 {"k":"\u00e9"}"#, r#"\ud800 {"k":"\u00e9"}"#),
  ];
  for (input, output) in cases {
    assert_eq!(sanitize_surrogates(input), output, "{input}");
  }
}

#[test]
fn documents_parse_in_javascript_key_order_or_say_why_not() {
  let parsed = doc(r#"{"b":1,"2":2,"a":{"z":0,"0":1}}"#).unwrap();
  assert_eq!(parsed.to_text(false), r#"{"2":2,"b":1,"a":{"0":1,"z":0}}"#);
  assert_eq!(parse_document("not json"), Err(Unreadable::NotJson));
  let deep = format!("{}{}", "[".repeat(200), "]".repeat(200));
  assert_eq!(parse_document(&deep), Err(Unreadable::TooDeep));
}

#[test]
fn fields_read_like_jq_raw_with_the_empty_default() {
  let d = doc(r#"{"a":{"s":"text\n\n","n":5,"f":false,"z":null,"o":{"k":1}},"arr":[1]}"#);
  assert_eq!(read_field(d.as_ref(), &["a", "s"]), "text");
  assert_eq!(read_field(d.as_ref(), &["a", "n"]), "5");
  assert_eq!(read_field(d.as_ref(), &["a", "o"]), "{\n  \"k\": 1\n}");
  for path in [
    &["a", "f"][..],
    &["a", "z"],
    &["a", "missing"],
    &["arr", "x"],
    &["a", "s", "deeper"],
  ] {
    assert_eq!(read_field(d.as_ref(), path), "", "{path:?}");
  }
  assert_eq!(read_field(None, &["a"]), "");
}

#[test]
fn advisories_are_deduplicated_and_merged_under_the_event() {
  let mut advisories = Advisories::default();
  for text in [
    r#"{"hookSpecificOutput":{"additionalContext":"one"},"systemMessage":"m"}"#,
    r#"{"hookSpecificOutput":{"additionalContext":"one"}}"#,
    r#"{"hookSpecificOutput":{"additionalContext":"two"},"systemMessage":"m"}"#,
  ] {
    advisories.collect(doc(text).as_ref());
  }
  assert_eq!(
    final_advisory(&advisories, "PostToolUse"),
    "{\n  \"hookSpecificOutput\": {\n    \"hookEventName\": \"PostToolUse\",\n    \"additionalContext\": \"one\\n\\ntwo\"\n  },\n  \"systemMessage\": \"m\"\n}\n"
  );
  assert_eq!(final_advisory(&Advisories::default(), "PreToolUse"), "");
}

#[test]
fn a_held_ask_takes_the_advisories_or_is_printed_as_written() {
  let advisories = Advisories {
    contexts: vec!["ctx".to_owned()],
    messages: vec!["msg".to_owned()],
  };
  let ask = r#"{"hookSpecificOutput":{"permissionDecision":"ask","permissionDecisionReason":"why"},"systemMessage":"was"}"#;
  let merged = final_ask(ask, &advisories);
  let value: serde_json::Value = serde_json::from_str(&merged).unwrap();
  assert_eq!(
    value["hookSpecificOutput"]["permissionDecisionReason"],
    "why\n\nctx"
  );
  assert_eq!(value["systemMessage"], "was\n\nmsg");
  let numeric =
    r#"{"hookSpecificOutput":{"permissionDecision":"ask","permissionDecisionReason":5}}"#;
  assert_eq!(final_ask(numeric, &advisories), printed(numeric));
  let bad_message = r#"{"hookSpecificOutput":{"permissionDecision":"ask"},"systemMessage":7}"#;
  assert_eq!(final_ask(bad_message, &advisories), printed(bad_message));
  assert_eq!(final_ask("not json", &advisories), "not json\n");
  let bare = r#"{"hookSpecificOutput":{"permissionDecision":"ask"}}"#;
  let quiet = final_ask(bare, &Advisories::default());
  assert!(
    quiet.contains("\"permissionDecisionReason\": \"\""),
    "{quiet}"
  );
  assert_eq!(substituted("x\n\n"), "x");
}
