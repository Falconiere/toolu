//! The jq helpers against the real jq binary (`ledger-jq.test.ts`): each case
//! runs the filter the bash libraries used and the helper over the same input,
//! and both succeed with the same value or both fail.

use toolu_runtime::json::jq_text;
use toolu_runtime::json::ordered::Ordered;
use toolu_runtime::process::{Spec, run};

use super::{
  JqError, alt, assign, concat, each, equals, get, holds, index, index_of, length, number,
  parse_json, raw, string, to_str, truthy,
};

const SHAPES: &[&str] = &[
  "null",
  "false",
  "true",
  "0",
  "7",
  "-2.5",
  "\"\"",
  "\"AC-1 AC-2\"",
  "[]",
  "[\"AC-1\",3,null]",
  "{}",
  "{\"a\":1,\"status\":\"green\",\"id\":\"s1\"}",
  "{\"AC-1\":[1]}",
  "{\"AC-1\":[null]}",
  "{\"AC-1\":\"x\"}",
];

/// `jq <flags> <filter>` over `input`: its output without the final newline, or `None` on failure.
fn jq(filter: &str, input: &str, flags: &[&str]) -> Option<String> {
  let mut spec = Spec::new(["jq"]);
  spec
    .argv
    .extend(flags.iter().map(|flag| (*flag).to_owned()));
  spec.argv.push(filter.to_owned());
  spec.stdin = input.as_bytes().to_vec();
  let output = run(&spec).unwrap();
  (output.exit_code == 0).then(|| output.stdout.trim_end_matches('\n').to_owned())
}

fn compact(value: &Ordered) -> String {
  jq_text(value, false)
}

fn ours(result: Result<String, JqError>) -> Option<String> {
  result.ok()
}

fn value(text: &str) -> Ordered {
  parse_json(text).unwrap()
}

#[test]
fn get_matches_jq_dot_status() {
  for shape in SHAPES {
    let input = value(shape);
    let got = ours(get(&input, "status").map(compact));
    assert_eq!(got, jq(".status", shape, &["-c"]), "{shape}");
  }
}

#[test]
fn each_matches_jq_iteration() {
  for shape in SHAPES {
    let input = value(shape);
    let got =
      each(&input).map(|items| compact(&Ordered::Array(items.into_iter().cloned().collect())));
    assert_eq!(ours(got), jq("[.[]]", shape, &["-c"]), "{shape}");
  }
}

#[test]
fn alt_matches_jq_alternative() {
  let fallback = string("d");
  for shape in SHAPES {
    let input = value(shape);
    let got = compact(alt(&input, &fallback));
    assert_eq!(Some(got), jq(". // \"d\"", shape, &["-c"]), "{shape}");
  }
}

#[test]
fn length_matches_jq() {
  for shape in SHAPES {
    let input = value(shape);
    let got = length(&input).map(|n| compact(&number(n)));
    assert_eq!(ours(got), jq("length", shape, &["-c"]), "{shape}");
  }
}

#[test]
fn holds_matches_jq_index_truthiness() {
  for shape in SHAPES {
    let input = value(shape);
    let got = holds(&input, "AC-1").map(|found| found.to_string());
    let filter = "if index(\"AC-1\") then true else false end";
    assert_eq!(ours(got), jq(filter, shape, &["-c"]), "{shape}");
  }
}

#[test]
fn to_str_and_raw_match_jq() {
  let extra = ["\"a\\u007fb\"", "{\"nested\":[\"x\",{\"y\":null}]}"];
  for shape in SHAPES.iter().chain(extra.iter()) {
    let input = value(shape);
    let tostring = jq_text(&string(&to_str(&input)), false);
    assert_eq!(Some(tostring), jq("tostring", shape, &["-c"]), "{shape}");
    assert_eq!(Some(raw(&input)), jq(".", shape, &["-r"]), "{shape}");
  }
}

#[test]
fn concat_matches_jq_string_addition() {
  let prefix = string("p");
  for shape in SHAPES {
    let input = value(shape);
    let got = concat(&[&prefix, &input]).map(|text| compact(&string(&text)));
    assert_eq!(ours(got), jq("\"p\" + .", shape, &["-c"]), "{shape}");
  }
}

#[test]
fn index_with_a_value_key_needs_an_object_and_a_string() {
  let object = value("{\"s1\":\"x\"}");
  assert_eq!(index(&object, &string("s1")), Ok(&string("x")));
  assert_eq!(index(&Ordered::Null, &value("3")), Ok(&Ordered::Null));
  assert!(index(&object, &value("3")).is_err());
  assert!(index(&value("[1]"), &string("s1")).is_err());
}

#[test]
fn index_of_reports_positions_and_first_elements() {
  assert_eq!(
    index_of(&value("[\"a\",\"AC-1\"]"), "AC-1"),
    Ok(number(1.0))
  );
  assert_eq!(index_of(&value("\"xAC-1\""), "AC-1"), Ok(number(1.0)));
  assert_eq!(
    index_of(&value("{\"AC-1\":[false]}"), "AC-1"),
    Ok(Ordered::Bool(false))
  );
  assert_eq!(index_of(&value("{\"AC-1\":[]}"), "AC-1"), Ok(Ordered::Null));
  assert!(index_of(&value("true"), "AC-1").is_err());
}

#[test]
fn equality_is_structural() {
  assert!(equals(
    &value("{\"a\":1,\"b\":[2]}"),
    &value("{\"b\":[2],\"a\":1.0}")
  ));
  assert!(!equals(&value("{\"a\":1}"), &value("{\"a\":1,\"b\":2}")));
  assert!(!equals(&value("[1]"), &value("[1,2]")));
  assert!(!equals(&value("\"1\""), &value("1")));
  assert!(truthy(&value("0")) && !truthy(&value("false")));
}

#[test]
fn assign_keeps_an_existing_key_in_place_and_refuses_non_objects() {
  let object = value("{\"a\":1,\"b\":2}");
  assert_eq!(
    compact(&assign(&object, "a", number(3.0)).unwrap()),
    "{\"a\":3,\"b\":2}"
  );
  assert_eq!(
    compact(&assign(&Ordered::Null, "x", Ordered::Null).unwrap()),
    "{\"x\":null}"
  );
  assert!(assign(&value("[]"), "a", Ordered::Null).is_err());
}

#[test]
fn parse_json_orders_integer_keys_first_and_rejects_garbage() {
  assert_eq!(
    compact(&value("{\"b\":1,\"2\":0,\"1\":0}")),
    "{\"1\":0,\"2\":0,\"b\":1}"
  );
  assert_eq!(parse_json("{"), None);
  assert_eq!(number(f64::NAN), Ordered::Null);
}
