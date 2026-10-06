use crate::markdown_cli::shell::lex;

fn words(text: &str) -> Vec<Vec<String>> {
  lex(text, 1)
    .commands
    .into_iter()
    .map(|command| command.words)
    .collect()
}

#[test]
fn redirections_drop_with_their_target_and_descriptor() {
  assert_eq!(
    words("gh pr view 2>/dev/null >out.txt\n"),
    [["gh", "pr", "view"]]
  );
  assert_eq!(words("jq . <in.json 2>&1 >> \"$LOG\"\n"), [["jq", "."]]);
  assert_eq!(words("cmd &>/dev/null\n"), [["cmd"]]);
}

#[test]
fn placeholders_are_words_not_redirections() {
  assert_eq!(
    words("toolu hook <name> --event <Event>\n"),
    [["toolu", "hook", "<name>", "--event", "<Event>"]]
  );
  assert_eq!(words("cat <[x]>\n"), [["cat", "<[x]>"]]);
  assert_eq!(words("sort < in\n"), [["sort"]]);
}

#[test]
fn heredoc_bodies_are_skipped_in_every_delimiter_form() {
  for opener in ["<<EOF", "<<'EOF'", "<<\"EOF\"", "<<-EOF"] {
    let text = format!("cat {opener}\ngit push\n  EOF\ngh x\n");
    assert_eq!(words(&text), [vec!["cat"], vec!["gh", "x"]], "{opener}");
  }
  assert_eq!(words("cat <<<\"here string\"\n"), [["cat"]]);
}
