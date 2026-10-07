//! Simple commands: the forms unbash reads as plain commands, keywords, the
//! text of a command, and reserved words out of place.

use crate::analysis::ShellCommand;
use crate::analyze;

fn commands(source: &str) -> Vec<ShellCommand> {
  analyze(source).commands
}

fn words(command: &ShellCommand) -> Vec<Option<&str>> {
  command.words.iter().map(Option::as_deref).collect()
}

#[test]
fn an_assignment_alone_is_a_command_without_words() {
  let found = commands("A=1");
  assert_eq!(
    (
      words(&found[0]),
      found[0].text.as_str(),
      found[0].exit_proves
    ),
    (vec![], "A=1", true)
  );
  let prefixed = commands("x=1 git push");
  assert_eq!(prefixed[0].text, "x=1 git push");
  assert_eq!(words(&prefixed[0]), [Some("git"), Some("push")]);
  assert_eq!(commands("A=1 B=2").len(), 1);
}

#[test]
fn declarations_and_unset_take_their_keyword_as_the_first_word() {
  assert_eq!(
    words(&commands("export A=$(x) B=2")[1]),
    [Some("export"), None, Some("B=2")]
  );
  assert_eq!(
    words(&commands("local a=1")[0]),
    [Some("local"), Some("a=1")]
  );
  assert_eq!(words(&commands("unset x")[0]), [Some("unset"), Some("x")]);
}

#[test]
fn a_single_bracket_test_is_a_command_and_a_double_one_is_not() {
  let found = commands("[ -f \"$x\" ] || [ a = b ]");
  assert_eq!(words(&found[0]), [Some("["), Some("-f"), None, Some("]")]);
  assert_eq!(
    words(&found[1]),
    [Some("["), Some("a"), Some("="), Some("b"), Some("]")]
  );
  assert_eq!(found[0].text, "[ -f \"$x\" ]");
  assert_eq!(commands("[[ -f x ]]"), Vec::<ShellCommand>::new());
  assert_eq!(commands("(( x > 1 ))"), Vec::<ShellCommand>::new());
}

#[test]
fn coproc_runs_its_command_in_the_background() {
  let found = commands("coproc git push");
  assert_eq!(
    (words(&found[0]), found[0].exit_proves),
    (vec![Some("git"), Some("push")], false)
  );
  assert_eq!(found[0].text, "git push");
  assert_eq!(words(&commands("coproc")[0]), [Some("coproc")]);
}

#[test]
fn a_closing_reserved_word_as_a_command_is_a_syntax_error() {
  for source in ["fi", "then", "done", "esac", "elif x", "else"] {
    let analysis = analyze(source);
    assert!(
      analysis.unknown && analysis.errors[0].message.starts_with("unexpected token"),
      "{source}"
    );
  }
}

#[test]
fn a_command_text_spans_its_redirects_but_no_heredoc_body() {
  assert_eq!(commands("echo x > .en[v]")[0].text, "echo x > .en[v]");
  assert_eq!(
    commands("cat <<'EOF' >f\nbody\nEOF")[0].text,
    "cat <<'EOF' >f"
  );
  assert_eq!(commands("> out.txt")[0].text, "> out.txt");
  assert_eq!(commands("node -\\\ne x")[0].text, "node -\\\ne x");
}

#[test]
fn statements_hung_under_a_command_still_run() {
  let found = commands("time (git push)");
  assert_eq!(words(&found[0]), [Some("git"), Some("push")]);
  assert!(found[0].exit_proves);
}
