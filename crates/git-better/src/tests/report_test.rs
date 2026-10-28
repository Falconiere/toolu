//! How findings read to the agent.

use super::{Finding, Target, render};
use crate::format::Violation;

#[test]
fn each_finding_lists_its_checks_and_an_indented_example() {
  let findings = [
    Finding {
      target: Target::Message,
      name: String::new(),
      violations: vec![Violation::new("missing-intent", "add an `Intent:` trailer")],
      example: "fix(x): y\n\nIntent: <why>".to_owned(),
    },
    Finding {
      target: Target::Branch,
      name: "my-branch".to_owned(),
      violations: vec![Violation::new("branch-format", "not semantic")],
      example: "<type>/my-branch".to_owned(),
    },
  ];
  let text = render("headline", &findings);
  assert_eq!(
    text,
    "headline\nCommit message:\n- add an `Intent:` trailer\nWrite it as:\n  fix(x): y\n\n  Intent: <why>\nBranch `my-branch`:\n- not semantic\nWrite it as:\n  <type>/my-branch\nCheck first with `toolu git-better check --message-file <file>` or `toolu git-better check --branch <name>`."
  );
  assert_eq!(
    (Target::Message.as_str(), Target::Branch.as_str()),
    ("message", "branch")
  );
}
