use serde_json::json;

use super::salvage;

#[test]
fn closed_objects_survive_a_cut_and_the_open_tail_counts_once() {
  let cut = r#"{"findings": [{"claim": "a"}, {"claim": "b"}, {"claim": "unterminat"#;
  let (values, dropped) = salvage(cut).unwrap();
  assert_eq!(values, [json!({"claim": "a"}), json!({"claim": "b"})]);
  assert_eq!(dropped, 1);
}

#[test]
fn braces_and_quotes_inside_strings_do_not_end_an_object() {
  let text = r#"{"findings": [{"claim": "a } { \" ] b"}]"#;
  let (values, dropped) = salvage(text).unwrap();
  assert_eq!(values, [json!({"claim": "a } { \" ] b"})]);
  assert_eq!(dropped, 0);
}

#[test]
fn text_that_is_not_a_findings_object_salvages_nothing() {
  assert_eq!(salvage("sorry, no JSON here"), None);
  assert_eq!(salvage(r#"{"other": []}"#), None);
  // A closed object that is not valid JSON is dropped and counted.
  assert_eq!(salvage(r#"{"findings": [{bad}]}"#), Some((Vec::new(), 1)));
}
