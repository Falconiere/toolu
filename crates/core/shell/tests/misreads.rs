//! Valid bash that tree-sitter-bash 0.23 reads differently from bash without an
//! ERROR node, found in the pre-push review of #416: each is now read as
//! TypeScript reads it.

use toolu_shell::analysis::{ShellAnalysis, Tristate};
use toolu_shell::analyze;
use toolu_shell::git::runs_git_subcommand;
use toolu_shell::writes::write_targets;

fn argvs(analysis: &ShellAnalysis) -> Vec<Vec<Option<String>>> {
  analysis
    .commands
    .iter()
    .map(|command| command.argv.clone())
    .collect()
}

fn words(words: &[&str]) -> Vec<Option<String>> {
  words.iter().map(|word| Some((*word).to_owned())).collect()
}

fn paths(analysis: &ShellAnalysis) -> Vec<Option<String>> {
  write_targets(analysis)
    .into_iter()
    .map(|target| target.path)
    .collect()
}

#[test]
fn arithmetic_commands_keep_their_redirects() {
  let analysis = analyze("(( 1 )) > .env");
  assert_eq!(argvs(&analysis), Vec::<Vec<Option<String>>>::new());
  assert_eq!(paths(&analysis), [Some(".env".to_owned())]);
  let test_form = analyze("((x++)) 2>&1");
  assert_eq!(argvs(&test_form), Vec::<Vec<Option<String>>>::new());
  assert!(!test_form.unknown);
  let nested = analyze("(( 1 )) > $(touch x)");
  assert_eq!(argvs(&nested), [words(&["touch", "x"])]);
  assert_eq!(paths(&analyze("[[ a ]] > .env")), [Some(".env".to_owned())]);
}

#[test]
fn a_here_string_on_a_compound_statement_runs_its_substitution() {
  let analysis = analyze("while read l; do :; done <<< \"$(git push)\"");
  assert_eq!(runs_git_subcommand(&analysis, "push"), Tristate::Yes);
  assert_eq!(
    argvs(&analysis),
    [
      words(&["git", "push"]),
      words(&["read", "l"]),
      words(&[":"])
    ]
  );
}

#[test]
fn coproc_before_a_subshell_runs_the_subshell_in_the_background() {
  let analysis = analyze("coproc (git push)");
  let rows: Vec<(Vec<Option<String>>, bool)> = analysis
    .commands
    .into_iter()
    .map(|command| (command.argv, command.exit_proves))
    .collect();
  assert_eq!(rows, [(words(&["git", "push"]), false)]);
}

#[test]
fn a_heredoc_line_keeps_every_command_after_its_delimiter() {
  let chained = analyze("cat <<EOF && a | git push\nx\nEOF");
  assert_eq!(
    argvs(&chained),
    [words(&["cat"]), words(&["a"]), words(&["git", "push"])]
  );
  let assigned = analyze("a=1 <<-EOF >f rm -rf x && true\nx\nEOF\n");
  assert_eq!(
    argvs(&assigned),
    [words(&["rm", "-rf", "x"]), words(&["true"])]
  );
  assert_eq!(paths(&assigned), [Some("f".to_owned())]);
}

#[test]
fn a_digit_glued_to_a_heredoc_is_its_descriptor() {
  let analysis = analyze("0<<EOF git push\nx\nEOF\n");
  assert_eq!(argvs(&analysis), [words(&["git", "push"])]);
  let fds: Vec<Option<u32>> = analysis
    .commands
    .iter()
    .flat_map(|command| command.redirects.iter().map(|redirect| redirect.fd))
    .collect();
  assert_eq!(fds, [Some(0)]);
}

#[test]
fn a_bare_dollar_before_a_name_still_expands() {
  let analysis = analyze("echo \"y\"\\\"$b\\e");
  assert_eq!(argvs(&analysis), [vec![Some("echo".to_owned()), None]]);
}

#[test]
fn braces_expand_across_quotes() {
  let analysis = analyze("{\"git\",\"push\"} origin");
  assert_eq!(argvs(&analysis), [vec![None, Some("origin".to_owned())]]);
  assert_eq!(runs_git_subcommand(&analysis, "push"), Tristate::Unknown);
  assert_eq!(
    argvs(&analyze("echo \"{a,b}\"")),
    [words(&["echo", "{a,b}"])]
  );
  assert_eq!(
    argvs(&analyze("echo {\"a,b\"}")),
    [words(&["echo", "{a,b}"])]
  );
}

#[test]
fn indented_heredoc_bodies_and_multi_line_test_strings_stay_known() {
  for source in [
    "cat <<EOF\n  x\nEOF",
    "cat <<-EOF\n\tbody\nEOF",
    "cat <<EOF\n\nEOF",
    "[ x = \"a\nb\" ]",
    "[ -n \"$(\ngit status\n)\" ]",
  ] {
    let analysis = analyze(source);
    assert!(!analysis.unknown, "{source:?} {:?}", analysis.errors);
  }
  // tree-sitter starts the body after the indentation; bash keeps it.
  let commit = analyze("git commit -m \"$(cat <<'EOF'\n  Subject\nEOF\n)\"");
  assert_eq!(
    argvs(&commit).last(),
    Some(&words(&["git", "commit", "-m", "  Subject"]))
  );
}
