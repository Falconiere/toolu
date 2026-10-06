//! Nested scripts: substitutions, re-parsed strings and backtick bodies.

use crate::analysis::CommandOrigin;
use crate::analyze;

fn origins(source: &str) -> Vec<(Option<String>, CommandOrigin, usize)> {
  let analysis = analyze(source);
  analysis
    .commands
    .into_iter()
    .map(|c| (c.argv.first().cloned().flatten(), c.origin, c.depth))
    .collect()
}

fn named(
  name: &str,
  origin: CommandOrigin,
  depth: usize,
) -> (Option<String>, CommandOrigin, usize) {
  (Some(name.to_owned()), origin, depth)
}

#[test]
fn a_substitution_runs_before_its_command_and_proves_nothing() {
  let analysis = analyze("echo $(git push) `date` <(ls)");
  let names: Vec<Option<String>> = analysis
    .commands
    .iter()
    .map(|c| c.argv[0].clone())
    .collect();
  assert_eq!(
    names,
    ["git", "date", "ls", "echo"].map(|n| Some(n.to_owned()))
  );
  assert!(analysis.commands.iter().take(3).all(|c| !c.exit_proves));
}

#[test]
fn a_re_parsed_string_keeps_its_origin_and_depth() {
  assert_eq!(
    origins("eval 'bash -c \"git push\"'"),
    [
      named("eval", CommandOrigin::Line, 0),
      named("bash", CommandOrigin::Eval, 1),
      named("git", CommandOrigin::Shell, 2)
    ]
  );
}

#[test]
fn a_decoded_backtick_body_is_parsed_with_its_own_source() {
  let analysis = analyze("echo `echo \\$HOME \\`whoami\\``");
  let names: Vec<Option<String>> = analysis
    .commands
    .iter()
    .map(|c| c.argv[0].clone())
    .collect();
  assert_eq!(
    names,
    ["whoami", "echo", "echo"].map(|n| Some(n.to_owned()))
  );
  assert_eq!(analysis.commands[1].texts[1], "$HOME");
}

#[test]
fn a_cancelled_nested_parse_makes_the_line_unknown() {
  let source = format!("bash -c '{}'", "${".repeat(524_288));
  let analysis = analyze(&source);
  assert!(analysis.unknown);
}

#[test]
fn compound_redirect_targets_run_their_substitutions() {
  let analysis = analyze("{ x; } > \"$(git push)\"");
  assert_eq!(analysis.commands[0].argv[0].as_deref(), Some("git"));
  assert_eq!(analysis.compound_redirects.len(), 1);
}
