//! The push-review v2 reviewer and coverage checks (`review-state.ts`).

use super::{
  ACCEPTED_REVIEWERS, has_accepted_reviewer, output_lines, reviewed_files, sorted_unique,
};
use crate::ledger::jq::parse_json;

fn lines(items: &[&str]) -> Vec<String> {
  items.iter().map(|item| (*item).to_owned()).collect()
}

#[test]
fn a_reviewer_is_accepted_wherever_jq_index_finds_it() {
  let accepted = |text: &str| has_accepted_reviewer(&parse_json(text).unwrap());
  assert!(accepted(r#"{"reviewers":["toolu-review:review"]}"#));
  assert!(
    accepted(r#"{"reviewers":"my code-review run"}"#),
    "a string is searched"
  );
  assert!(
    accepted(r#"{"reviewers":{"review":["x"]}}"#),
    "an object yields its first element"
  );
  assert!(!accepted(r#"{"reviewers":{"review":[]}}"#));
  assert!(!accepted(r#"{"reviewers":["someone"]}"#));
  assert!(!accepted(r#"{"reviewers":null}"#));
  assert!(!accepted(r#"{"reviewers":3}"#), "a jq error is no");
  assert!(!accepted("[]"));
  assert_eq!(ACCEPTED_REVIEWERS.len(), 5);
}

#[test]
fn sort_unique_and_output_lines_follow_the_shell() {
  assert_eq!(sorted_unique(&lines(&["b", "a", "b", ""])), "\na\nb");
  assert_eq!(sorted_unique(&lines(&[])), "");
  assert_eq!(output_lines("a\nb\n"), lines(&["a", "b"]));
  assert_eq!(output_lines("a\n\n"), lines(&["a", ""]));
  assert_eq!(output_lines(""), Vec::<String>::new());
}

#[test]
fn reviewed_files_are_the_raw_lines_of_each_entry() {
  let files = |text: &str| reviewed_files(&parse_json(text).unwrap());
  assert_eq!(
    files(r#"{"reviewed_files":["a.ts","b\nc",3]}"#),
    lines(&["a.ts", "b", "c", "3"])
  );
  assert_eq!(files(r#"{"reviewed_files":null}"#), Vec::<String>::new());
  assert_eq!(files(r#"{"reviewed_files":"a.ts"}"#), Vec::<String>::new());
  assert_eq!(files("[1]"), Vec::<String>::new());
}
