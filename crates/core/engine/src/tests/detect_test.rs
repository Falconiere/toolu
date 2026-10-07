use toolu_shell::analyze;

use super::{is_git_commit, is_git_push};

#[test]
fn unknown_to_the_shell_layer_maps_to_false() {
  for command in ["$g push", "git $(echo push)"] {
    assert!(!is_git_push(&analyze(command)), "{command}");
  }
  assert!(!is_git_commit(&analyze("\"$GIT\" commit -m x")));
  assert!(is_git_push(&analyze("git -C repo push origin main")));
  assert!(is_git_commit(&analyze("echo a && git commit -m x")));
  assert!(!is_git_push(&analyze("git status")));
}

#[test]
fn a_heredoc_body_runs_nothing() {
  let lines = |parts: &[&str]| parts.join("\n");
  let cases = [
    (
      lines(&[
        "cat <<EOF > /tmp/x",
        "git push origin main",
        "EOF",
        "echo after",
      ]),
      false,
    ),
    (
      lines(&["cat <<-END", "\tgit push", "\tEND", "echo end"]),
      false,
    ),
    (lines(&["cat <<DOC", "git push", "DOC", "echo end"]), false),
    (lines(&["echo hello", "ls -la", "git push"]), true),
    (
      lines(&[
        "cat <<EOF | tee /tmp/x",
        "secret git push inside body",
        "EOF",
        "echo done",
      ]),
      false,
    ),
  ];
  for (command, push) in cases {
    assert_eq!(is_git_push(&analyze(&command)), push, "{command}");
  }
}
