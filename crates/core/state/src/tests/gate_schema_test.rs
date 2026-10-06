use toolu_runtime::json::ordered::Ordered;

use super::{GateEntry, GateFile, validate_gate_file};

fn json(text: &str) -> Ordered {
  Ordered::parse(text).unwrap()
}

const ENTRY: &str =
  r#"{"source":"s","reason":"r","violations":"v\n","updatedAt":"2020-01-01T00:00:00Z"}"#;

fn failing(extra: &str) -> String {
  format!(
    r#"{{"status":"failing","reason":"r","source":"s","file":"/a","violations":"v\n","entries":{{"/a":{ENTRY}}},"updatedAt":"u"{extra}}}"#
  )
}

#[test]
fn passing_legacy_multi_slot_and_version_1_documents_are_valid() {
  let passing = validate_gate_file(&json(
    r#"{"status":"passing","source":"s","updatedAt":"u"}"#,
  ));
  assert_eq!(
    passing,
    Ok(GateFile::Passing {
      source: "s".into(),
      updated_at: "u".into()
    })
  );
  let legacy = r#"{"status":"failing","reason":"r","source":"s","file":"/a","violations":"v","updatedAt":"u"}"#;
  assert!(matches!(
    validate_gate_file(&json(legacy)),
    Ok(GateFile::Failing { entries: None, .. })
  ));
  let Ok(GateFile::Failing {
    entries: Some(entries),
    ..
  }) = validate_gate_file(&json(&failing("")))
  else {
    panic!("not multi-slot");
  };
  let entry = GateEntry {
    source: "s".into(),
    reason: "r".into(),
    violations: "v\n".into(),
    updated_at: "2020-01-01T00:00:00Z".into(),
  };
  assert_eq!(entries, [("/a".to_owned(), entry)]);
  assert!(validate_gate_file(&json(&failing(r#","version":1"#))).is_ok());
  assert!(validate_gate_file(&json(&failing(r#","version":1.0"#))).is_ok());
}

#[test]
fn documents_outside_v1_name_their_first_issue() {
  let cases = [
    (failing(r#","owner":"x""#), r#"(root): Unrecognized key: "owner""#.to_owned()),
    (failing(r#","a":1,"b":2"#), r#"(root): Unrecognized keys: "a", "b""#.to_owned()),
    (failing(r#","version":2"#), "version: Invalid input: expected 1".to_owned()),
    (failing(r#","version":null"#), "version: Invalid input: expected 1".to_owned()),
    (r#"{"status":"passing","source":"s","updatedAt":"u","x":1}"#.to_owned(), r#"(root): Unrecognized key: "x""#.to_owned()),
    (r#"{"status":"unknown"}"#.to_owned(), "status: Invalid input".to_owned()),
    (r#"{"source":"s"}"#.to_owned(), "status: Invalid input".to_owned()),
    ("[]".to_owned(), "(root): Invalid input: expected object, received array".to_owned()),
    ("true".to_owned(), "(root): Invalid input: expected object, received boolean".to_owned()),
    (
      r#"{"status":"failing","reason":"r","source":"s","file":"/a","violations":1,"updatedAt":"u"}"#.to_owned(),
      "violations: Invalid input: expected string, received number".to_owned(),
    ),
    (
      r#"{"status":"failing","reason":"r","source":"s","file":"/a","violations":"v","entries":[],"updatedAt":"u"}"#.to_owned(),
      "entries: Invalid input: expected record, received array".to_owned(),
    ),
    (
      r#"{"status":"failing","reason":"r","source":"s","file":"/a","violations":"v","entries":{"a":"x"},"updatedAt":"u"}"#.to_owned(),
      "entries.a: Invalid input: expected object, received string".to_owned(),
    ),
    (
      r#"{"status":"passing","updatedAt":"u"}"#.to_owned(),
      "source: Invalid input: expected string, received undefined".to_owned(),
    ),
  ];
  for (text, reason) in cases {
    assert_eq!(validate_gate_file(&json(&text)), Err(reason), "{text}");
  }
  let nested = failing("").replace(
    r#""updatedAt":"2020-01-01T00:00:00Z"}"#,
    r#""updatedAt":"t","x":1}"#,
  );
  assert_eq!(
    validate_gate_file(&json(&nested)),
    Err(r#"entries./a: Unrecognized key: "x""#.to_owned())
  );
}

#[test]
fn a_document_round_trips_in_the_writers_key_order() {
  let text = failing("");
  let doc = validate_gate_file(&json(&text)).unwrap();
  assert_eq!(doc.to_ordered().to_text(false), text);
  let passing = r#"{"status":"passing","source":"s","updatedAt":"u"}"#;
  assert_eq!(
    validate_gate_file(&json(passing))
      .unwrap()
      .to_ordered()
      .to_text(false),
    passing
  );
}

#[test]
fn unknown_keys_are_named_in_javascript_key_order() {
  let text = r#"{"status":"passing","source":"s","updatedAt":"u","b":1,"5":2}"#;
  assert_eq!(
    validate_gate_file(&json(text)),
    Err(r#"(root): Unrecognized keys: "5", "b""#.to_owned())
  );
}
