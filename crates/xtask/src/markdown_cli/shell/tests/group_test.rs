use crate::markdown_cli::shell::lex;

fn first_words(text: &str) -> Vec<String> {
  lex(text, 1)
    .commands
    .into_iter()
    .map(|command| command.words.join(" "))
    .collect()
}

#[test]
fn a_substitution_runs_inside_its_outer_command() {
  assert_eq!(
    first_words("gh pr view \"$(git branch)\" $(git rev-parse HEAD) --json x\n"),
    [
      "git rev-parse HEAD",
      "gh pr view \"$(git branch)\" --json x",
    ]
  );
}

#[test]
fn subshells_functions_and_braced_variables() {
  assert_eq!(
    first_words("(cd a && make)\nsetup () { :; }\necho ${A:-b}\n"),
    ["cd a", "make", ":", "echo ${A:-b}"]
  );
  assert_eq!(lex("setup () { :; }\n", 1).functions, ["setup"]);
}

#[test]
fn an_array_assignment_keeps_its_elements_out_of_commands() {
  let lexed = lex("A=(x y) B=1 gh pr view\n", 1);
  assert_eq!(lexed.commands[0].words, ["gh", "pr", "view"]);
}
