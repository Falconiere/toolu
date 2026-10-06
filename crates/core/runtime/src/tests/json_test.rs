use serde_json::{Value, json};

use super::{jq_text, js_number, stringify, stringify_pretty, top_level_keys};

#[test]
fn numbers_print_as_javascript_prints_them() {
  let cases = [
    (1.0, "1"),
    (2.5, "2.5"),
    (45.9, "45.9"),
    (1e21, "1e+21"),
    (1e20, "100000000000000000000"),
    (1e-7, "1e-7"),
    (0.000_001, "0.000001"),
    (-0.0, "0"),
    (12_345_678_901_234_567_000.0, "12345678901234567000"),
    (0.1, "0.1"),
    (123e-20, "1.23e-18"),
    (1.5e300, "1.5e+300"),
    (-3.25, "-3.25"),
    (100.0, "100"),
    (123_456.789, "123456.789"),
  ];
  for (number, text) in cases {
    assert_eq!(js_number(number), text, "{number:e}");
  }
  assert_eq!(js_number(f64::NAN), "NaN");
  assert_eq!(js_number(f64::INFINITY), "Infinity");
  assert_eq!(js_number(f64::NEG_INFINITY), "-Infinity");
}

#[test]
fn compact_text_is_json_stringify() {
  let value: Value =
    serde_json::from_str(r#"[[],{},"é",1.0,12345678901234567890,true,null]"#).unwrap();
  assert_eq!(
    stringify(&value),
    r#"[[],{},"é",1,12345678901234567000,true,null]"#
  );
}

#[test]
fn pretty_text_indents_two_spaces_and_keeps_empty_containers_inline() {
  let value = json!({ "b": [1, {}], "a": "x\u{7f}\n\u{1}", "c": [] });
  let expected =
    "{\n  \"a\": \"x\u{7f}\\n\\u0001\",\n  \"b\": [\n    1,\n    {}\n  ],\n  \"c\": []\n}";
  assert_eq!(stringify_pretty(&value), expected);
}

#[test]
fn jq_text_escapes_del_only() {
  let value = json!({ "a": "x\u{7f}y" });
  assert_eq!(jq_text(&value, false), r#"{"a":"x\u007fy"}"#);
  assert_eq!(jq_text(&value, true), "{\n  \"a\": \"x\\u007fy\"\n}");
}

#[test]
fn top_level_keys_keep_document_order_and_first_position() {
  let keys = top_level_keys(r#"{"zeta":1,"alpha":{"nested":2},"zeta":3,"mid":[]}"#);
  assert_eq!(keys.unwrap(), ["zeta", "alpha", "mid"]);
  assert_eq!(top_level_keys("{}").unwrap(), Vec::<String>::new());
}

#[test]
fn top_level_keys_are_none_for_anything_but_an_object() {
  for text in ["[1]", "null", "1", "\"x\"", "{", ""] {
    assert_eq!(top_level_keys(text), None, "{text}");
  }
}
