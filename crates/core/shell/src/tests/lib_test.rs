//! `shell-parse.test.ts`: the commands tree-sitter could read are reported,
//! errors come from every nested script, a line with no command at all is
//! unknown, and oversize or slow input is not analyzed.

use crate::analysis::CommandOrigin;
use crate::{MAX_SHELL_INPUT, analyze};

fn argvs(source: &str) -> Vec<Vec<Option<String>>> {
  analyze(source)
    .commands
    .into_iter()
    .map(|c| c.argv)
    .collect()
}

fn words(list: &[&str]) -> Vec<Option<String>> {
  list.iter().map(|word| Some((*word).to_owned())).collect()
}

#[test]
fn a_valid_command_followed_by_a_syntax_error_is_still_reported() {
  let analysis = analyze("git push; echo \"unterminated");
  assert!(
    analysis
      .commands
      .iter()
      .any(|c| c.argv == words(&["git", "push"]))
  );
  assert_ne!(analysis.errors, Vec::<crate::analysis::ShellError>::new());
  assert!(!analysis.unknown);
}

#[test]
fn a_line_bash_cannot_parse_proves_nothing() {
  let analysis = analyze("bun test &&");
  let rows: Vec<(Vec<Option<String>>, bool)> = analysis
    .commands
    .into_iter()
    .map(|c| (c.argv, c.exit_proves))
    .collect();
  assert_eq!(rows, [(words(&["bun", "test"]), false)]);
  assert!(analyze("bun test").commands[0].exit_proves);
}

#[test]
fn errors_with_no_command_at_all_make_the_line_unknown() {
  for source in [")", "if", "fi", "then", "done"] {
    let analysis = analyze(source);
    assert!(analysis.commands.is_empty(), "{source}");
    assert!(analysis.unknown, "{source}");
  }
}

#[test]
fn errors_inside_a_substitution_belong_to_it() {
  let analysis = analyze("echo $(git push; echo \"oops)");
  assert!(analysis.errors.iter().any(
    |error| error.origin == CommandOrigin::Substitution || error.origin == CommandOrigin::Line
  ));
  assert!(argvs("echo $(git push; echo \"oops)").contains(&words(&["git", "push"])));
  let inner = analyze("bash -c 'echo \"x'");
  assert!(
    inner
      .errors
      .iter()
      .any(|error| error.origin == CommandOrigin::Shell)
  );
}

#[test]
fn empty_and_whitespace_input_runs_nothing_and_is_not_unknown() {
  for source in ["", "   ", "\n\t\n"] {
    let analysis = analyze(source);
    assert!(
      analysis.commands.is_empty() && analysis.errors.is_empty() && !analysis.unknown,
      "{source:?}"
    );
    assert_eq!(analysis.source, source);
  }
}

#[test]
fn input_over_the_cap_is_not_parsed_and_is_unknown() {
  let source = format!("echo {}", "x".repeat(MAX_SHELL_INPUT));
  let analysis = analyze(&source);
  assert!(analysis.unknown);
  assert_eq!(
    analysis.commands,
    Vec::<crate::analysis::ShellCommand>::new()
  );
  let expected = format!(
    "oversize: {} characters exceeds the 1048576 cap",
    MAX_SHELL_INPUT + 5
  );
  assert_eq!(analysis.errors[0].message, expected);
  let wide = format!("echo {}", "é".repeat(MAX_SHELL_INPUT / 2));
  assert!(!analyze(&wide).unknown, "é is one UTF-16 unit, two bytes");
}

#[test]
fn input_at_the_cap_is_parsed() {
  let body = "x".repeat(MAX_SHELL_INPUT - "echo ".len());
  let analysis = analyze(&format!("echo {body}"));
  assert!(!analysis.unknown);
  assert_eq!(analysis.commands[0].argv[0].as_deref(), Some("echo"));
}

#[test]
fn a_heredoc_body_several_hundred_kib_long_stays_analyzable() {
  let body = "const x = 1; // not a command\n".repeat(20_000);
  let source = format!("cat > big.ts <<'EOF'\n{body}EOF\ngit add big.ts");
  assert_eq!(
    argvs(&source),
    [words(&["cat"]), words(&["git", "add", "big.ts"])]
  );
}

#[test]
fn a_parse_past_the_budget_is_cancelled_and_unknown() {
  let analysis = analyze(&"${".repeat(524_288));
  assert!(analysis.unknown);
  assert!(
    analysis
      .errors
      .iter()
      .any(|error| error.message.contains("parse budget"))
  );
}
