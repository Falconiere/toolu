use toolu_runtime::term::Style;

use super::terminal;

#[test]
fn a_run_that_may_not_ask_has_no_terminal() {
  assert!(terminal(false, Style::Color).is_none());
  assert!(terminal(false, Style::Plain).is_none());
}
