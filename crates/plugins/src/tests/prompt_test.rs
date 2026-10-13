use std::io::Cursor;

use super::{Ask, Choice, Prompter, picks, terminal};
use crate::fail::Fail;

fn options() -> Vec<Choice> {
  vec![
    ("toolu".to_owned(), Some("the hub".to_owned())),
    ("jev".to_owned(), None),
    ("ast-grep".to_owned(), None),
  ]
}

fn ask_many(input: &str) -> (Result<Vec<usize>, Fail>, String) {
  let mut out = Vec::new();
  let mut prompter = Prompter::new(Cursor::new(input.as_bytes().to_vec()), &mut out);
  let chosen = prompter.many("Which plugins?", &options());
  (chosen, String::from_utf8_lossy(&out).into_owned())
}

fn ask_one(input: &str) -> Result<usize, Fail> {
  let mut out = Vec::new();
  let mut prompter = Prompter::new(Cursor::new(input.as_bytes().to_vec()), &mut out);
  prompter.one("Which host?", &options())
}

#[test]
fn many_lists_numbered_options_and_enter_takes_them_all() {
  let (chosen, shown) = ask_many("\n");
  assert_eq!(chosen, Ok(vec![0, 1, 2]));
  assert_eq!(
    shown,
    "Which plugins?\n  1) toolu - the hub\n  2) jev\n  3) ast-grep\nNumbers to choose (Enter: all, q: cancel): "
  );
}

#[test]
fn many_takes_numbers_by_space_or_comma_and_re_asks_on_a_bad_answer() {
  assert_eq!(ask_many("1 3\n").0, Ok(vec![0, 2]));
  assert_eq!(ask_many("3,1,3\n").0, Ok(vec![2, 0]));
  let (chosen, shown) = ask_many("2,x\n9\n2\n");
  assert_eq!(chosen, Ok(vec![1]));
  assert_eq!(
    shown.matches("Answer with numbers from 1 to 3: ").count(),
    2
  );
}

#[test]
fn q_or_end_of_input_cancels() {
  for input in ["q\n", "", "9\n"] {
    let (chosen, _) = ask_many(input);
    assert_eq!(
      chosen.map_err(|fail| fail.message),
      Err("cancelled".to_owned()),
      "{input:?}"
    );
  }
  assert_eq!(
    ask_one("q\n").map_err(|fail| fail.message),
    Err("cancelled".to_owned())
  );
}

#[test]
fn one_takes_enter_as_the_first_and_exactly_one_number() {
  assert_eq!(ask_one("\n"), Ok(0));
  assert_eq!(ask_one("3\n"), Ok(2));
  assert_eq!(ask_one("1 2\n0\n2\n"), Ok(1));
}

#[test]
fn answers_outside_the_range_pick_nothing() {
  assert_eq!(picks("0", 3), None);
  assert_eq!(picks("4", 3), None);
  assert_eq!(picks(" , ", 3), None);
  assert_eq!(picks("2 2", 3), Some(vec![1]));
}

#[test]
fn a_run_that_may_not_ask_has_no_terminal() {
  assert!(terminal(false).is_none());
}
