use super::{EmptyText, Text};

#[test]
fn a_non_empty_string_is_text() {
  let text = Text::new("protected file").unwrap();
  assert_eq!(text.as_str(), "protected file");
  assert_eq!(String::from(text), "protected file");
}

#[test]
fn an_empty_string_is_refused_like_zod_min_1() {
  assert_eq!(Text::new(""), Err(EmptyText));
  assert_eq!(EmptyText.to_string(), "must not be empty");
}

#[test]
fn text_reads_and_writes_as_a_json_string() {
  let text: Text = serde_json::from_str(r#""a b""#).unwrap();
  assert_eq!(text.as_str(), "a b");
  assert_eq!(serde_json::to_string(&text).unwrap(), r#""a b""#);
  let err = serde_json::from_str::<Text>(r#""""#).unwrap_err();
  assert!(err.to_string().contains("must not be empty"), "{err}");
  assert!(serde_json::from_str::<Text>("7").is_err());
}
