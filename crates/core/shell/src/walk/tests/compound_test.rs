//! Compound commands: which bodies keep `proves`, which are scanned only, and
//! where their redirects go.

use crate::analysis::CommandOrigin;
use crate::analyze;

/// `(argv[0], exit proves, origin)` of every command.
fn rows(source: &str) -> Vec<(String, bool, CommandOrigin)> {
  let analysis = analyze(source);
  let name =
    |c: &crate::analysis::ShellCommand| c.argv.first().cloned().flatten().unwrap_or_default();
  analysis
    .commands
    .iter()
    .map(|c| (name(c), c.exit_proves, c.origin))
    .collect()
}

fn row(name: &str, proves: bool, origin: CommandOrigin) -> (String, bool, CommandOrigin) {
  (name.to_owned(), proves, origin)
}

#[test]
fn groups_and_subshells_keep_the_status_and_conditions_do_not() {
  use CommandOrigin::Line;
  assert_eq!(
    rows("{ a; b; }"),
    [row("a", false, Line), row("b", true, Line)]
  );
  assert_eq!(
    rows("(a; b)"),
    [row("a", false, Line), row("b", true, Line)]
  );
  assert_eq!(
    rows("if a; then b; elif c; then d; else e; fi"),
    ["a", "b", "c", "d", "e"].map(|n| row(n, false, Line))
  );
  assert_eq!(
    rows("until a; do b; done"),
    [row("a", false, Line), row("b", false, Line)]
  );
  assert_eq!(
    rows("for ((i=0;i<3;i++)); do x; done"),
    [row("x", false, Line)]
  );
  assert_eq!(
    rows("case $1 in a|b) x;; *) y;; esac"),
    [row("x", false, Line), row("y", false, Line)]
  );
}

#[test]
fn a_function_body_runs_only_when_called() {
  assert_eq!(
    rows("f() ( git push )"),
    [row("git", false, CommandOrigin::Function)]
  );
  assert_eq!(
    rows("function f { git push; }"),
    [row("git", false, CommandOrigin::Function)]
  );
}

#[test]
fn scanned_words_run_their_substitutions_first() {
  let found = rows("case $(a) in $(b)) c;; esac");
  let names: Vec<&str> = found.iter().map(|(name, _, _)| name.as_str()).collect();
  assert_eq!(names, ["a", "b", "c"]);
}

#[test]
fn redirects_on_a_compound_command_are_compound_redirects() {
  let analysis = analyze("while true; do x; done 2>err >>log");
  let targets: Vec<Option<&str>> = analysis
    .compound_redirects
    .iter()
    .map(|r| r.target.as_deref())
    .collect();
  assert_eq!(targets, [Some("err"), Some("log")]);
  assert_eq!(
    analysis.commands[0].redirects,
    Vec::<crate::analysis::ShellRedirect>::new()
  );
}
