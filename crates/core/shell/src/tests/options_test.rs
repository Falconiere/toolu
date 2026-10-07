use super::{OptionSpec, ParsedOption, Value, named, parse_args, value_at};
use crate::analysis::Word;

fn words(list: &[Option<&str>]) -> Vec<Word> {
  list.iter().map(|word| word.map(str::to_owned)).collect()
}

fn names<'w>(options: &[ParsedOption<'w>]) -> Vec<(&'w str, Value<'w>)> {
  options
    .iter()
    .map(|option| (option.name, option.value))
    .collect()
}

const COMMIT: OptionSpec = OptionSpec {
  value_short: "mFCct",
  rest_short: "Su",
  value_long: "message file",
  numeric: false,
  stop_at_operand: false,
  plus: false,
};

#[test]
fn a_short_cluster_reads_letter_by_letter_until_a_value_letter() {
  let list = words(&[Some("commit"), Some("-am"), Some("fix it"), Some("x")]);
  let parsed = parse_args(&list, 1, &COMMIT);
  assert_eq!(
    names(&parsed.options),
    [("a", Value::Absent), ("m", Value::Text("fix it"))]
  );
  assert_eq!(parsed.options[1].at, Some(2));
  assert_eq!(parsed.operand_at, [3]);
  assert_eq!(parsed.next, 4);
  assert!(!parsed.missing_value);
}

#[test]
fn a_value_letter_takes_the_rest_of_its_word() {
  let list = words(&[Some("x"), Some("-mmsg"), Some("-uno")]);
  let parsed = parse_args(&list, 1, &COMMIT);
  assert_eq!(
    names(&parsed.options),
    [("m", Value::Text("msg")), ("u", Value::Text("no"))]
  );
  assert_eq!(parsed.options[0].at, None);
}

#[test]
fn a_rest_letter_may_take_an_empty_value() {
  let spec = OptionSpec {
    rest_short: "i",
    ..OptionSpec::default()
  };
  let list = words(&[Some("sed"), Some("-i"), Some("s/a/b/"), Some("-i.bak")]);
  let parsed = parse_args(&list, 1, &spec);
  assert_eq!(
    names(&parsed.options),
    [("i", Value::Text("")), ("i", Value::Text(".bak"))]
  );
  assert_eq!(parsed.operand_at, [2]);
}

#[test]
fn long_options_take_an_equals_value_or_the_next_word_when_listed() {
  let list = words(&[
    Some("x"),
    Some("--message=a=b"),
    Some("--file"),
    Some("f.txt"),
    Some("--amend"),
  ]);
  let parsed = parse_args(&list, 1, &COMMIT);
  assert_eq!(
    names(&parsed.options),
    [
      ("message", Value::Text("a=b")),
      ("file", Value::Text("f.txt")),
      ("amend", Value::Absent),
    ]
  );
}

#[test]
fn a_value_option_at_the_end_is_a_missing_value() {
  let spec = OptionSpec {
    value_short: "C",
    stop_at_operand: true,
    ..OptionSpec::default()
  };
  let list = words(&[Some("git"), Some("-C")]);
  let parsed = parse_args(&list, 1, &spec);
  assert!(parsed.missing_value);
  assert_eq!(names(&parsed.options), [("C", Value::Absent)]);
}

#[test]
fn a_dynamic_value_word_is_dynamic() {
  let list = words(&[Some("git"), Some("-C"), None, Some("push")]);
  let spec = OptionSpec {
    value_short: "C",
    stop_at_operand: true,
    ..OptionSpec::default()
  };
  let parsed = parse_args(&list, 1, &spec);
  assert_eq!(names(&parsed.options), [("C", Value::Dynamic)]);
  assert_eq!(parsed.next, 3);
  assert_eq!(parsed.options[0].value.text(), None);
}

#[test]
fn numeric_options_are_one_option() {
  let spec = OptionSpec {
    value_short: "n",
    numeric: true,
    stop_at_operand: true,
    ..OptionSpec::default()
  };
  let list = words(&[Some("nice"), Some("-10"), Some("git")]);
  let parsed = parse_args(&list, 1, &spec);
  assert_eq!(names(&parsed.options), [("10", Value::Absent)]);
  assert_eq!(parsed.next, 2);
  let plain = parse_args(&list, 1, &OptionSpec::default());
  assert_eq!(
    names(&plain.options),
    [("1", Value::Absent), ("0", Value::Absent)]
  );
}

#[test]
fn plus_clusters_are_options_only_when_asked() {
  let spec = OptionSpec {
    value_short: "o",
    stop_at_operand: true,
    plus: true,
    ..OptionSpec::default()
  };
  let list = words(&[
    Some("bash"),
    Some("+o"),
    Some("posix"),
    Some("+c"),
    Some("x"),
  ]);
  let parsed = parse_args(&list, 1, &spec);
  assert_eq!(
    names(&parsed.options),
    [("o", Value::Text("posix")), ("c", Value::Absent)]
  );
  assert_eq!(parsed.next, 4);
  let plain = parse_args(
    &list,
    1,
    &OptionSpec {
      stop_at_operand: true,
      ..OptionSpec::default()
    },
  );
  assert_eq!(plain.next, 1);
  let lone = words(&[Some("bash"), Some("+"), Some("-")]);
  assert_eq!(parse_args(&lone, 1, &spec).next, 1);
}

#[test]
fn stopping_at_an_operand_skips_a_double_dash() {
  let spec = OptionSpec {
    stop_at_operand: true,
    ..OptionSpec::default()
  };
  for (list, next) in [
    (words(&[Some("x"), Some("-v"), Some("--"), Some("-y")]), 3),
    (words(&[Some("x"), Some("-v"), Some("-"), Some("y")]), 2),
    (words(&[Some("x"), None, Some("y")]), 1),
    (words(&[Some("x"), Some("-v")]), 2),
  ] {
    assert_eq!(parse_args(&list, 1, &spec).next, next, "{list:?}");
  }
}

#[test]
fn a_double_dash_makes_the_rest_operands() {
  let list = words(&[
    Some("rm"),
    Some("-f"),
    Some("--"),
    Some("-x"),
    None,
    Some("-"),
  ]);
  let parsed = parse_args(&list, 1, &OptionSpec::default());
  assert_eq!(names(&parsed.options), [("f", Value::Absent)]);
  assert_eq!(parsed.operand_at, [3, 4, 5]);
  assert_eq!(parsed.next, 6);
}

#[test]
fn has_and_values_read_options_by_name() {
  let list = words(&[
    Some("x"),
    Some("-m"),
    Some("a"),
    Some("--message"),
    Some("b"),
    Some("-q"),
  ]);
  let parsed = parse_args(&list, 1, &COMMIT);
  assert!(parsed.has("m message"));
  assert!(parsed.has("q"));
  assert!(!parsed.has("F file"));
  assert_eq!(
    parsed.values("m message"),
    [Value::Text("a"), Value::Text("b")]
  );
  assert_eq!(parsed.values("m message")[0].text(), Some("a"));
}

#[test]
fn names_match_whole_space_separated_entries() {
  assert!(named("user group", "group"));
  assert!(!named("user group", "grou"));
  assert!(!named("", "x"));
}

#[test]
fn value_at_distinguishes_absent_dynamic_and_text() {
  let list = words(&[Some("a"), None]);
  assert_eq!(value_at(&list, 0), Value::Text("a"));
  assert_eq!(value_at(&list, 1), Value::Dynamic);
  assert_eq!(value_at(&list, 2), Value::Absent);
}
