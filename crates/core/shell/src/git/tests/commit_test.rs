//! Every message source of a commit: `-m`, `-F` (with stdin), `--trailer`, and
//! the options that reuse or derive a message.

use super::{CommitArgs, MessageOrigin, commit_args};
use crate::analyze;
use crate::git::git_invocation;

fn args(source: &str) -> Option<CommitArgs> {
  let analysis = analyze(source);
  analysis
    .commands
    .iter()
    .filter_map(git_invocation)
    .find_map(|git| commit_args(&git))
}

fn some(values: &[&str]) -> Vec<Option<String>> {
  values
    .iter()
    .map(|value| Some((*value).to_owned()))
    .collect()
}

#[test]
fn only_a_commit_has_commit_args() {
  assert_eq!(args("git push -m x"), None);
  assert_eq!(args("git commit-tree -m x abc"), None);
  assert_eq!(args("git commit"), Some(CommitArgs::default()));
}

#[test]
fn messages_files_and_trailers_are_read_in_order() {
  let found = args(
    "git commit -m 'feat(x): y' --message=body -F msg.txt --file=- --trailer 'Intent: why' \
     --trailer \"$T\"",
  )
  .unwrap();
  assert_eq!(found.messages, some(&["feat(x): y", "body"]));
  assert_eq!(found.files, some(&["msg.txt", "-"]));
  assert_eq!(found.trailers, [Some("Intent: why".to_owned()), None]);
  assert_eq!(found.origin, MessageOrigin::Written);
  assert!(!found.amend);
}

#[test]
fn a_static_heredoc_or_herestring_on_stdin_is_read() {
  let heredoc = args("git commit -F - <<'EOF'\nfeat(x): y\n\nIntent: why\nEOF").unwrap();
  assert_eq!(heredoc.files, some(&["-"]));
  assert_eq!(
    heredoc.stdin.as_deref(),
    Some("feat(x): y\n\nIntent: why\n")
  );
  let here = args("git commit -F - <<< 'fix(y): z'").unwrap();
  assert_eq!(here.stdin.as_deref(), Some("fix(y): z"));
  let piped = args("printf 'fix: x' | git commit -F -").unwrap();
  assert_eq!(piped.stdin, None);
  let expands = args("git commit -F - <<EOF\n$MSG\nEOF").unwrap();
  assert_eq!(expands.stdin, None);
}

#[test]
fn reused_derived_and_kept_messages_are_told_apart() {
  let origin = |source: &str| args(source).unwrap().origin;
  for source in [
    "git commit -C HEAD",
    "git commit -c HEAD~1",
    "git commit --reuse-message=HEAD",
    "git commit --reedit-message HEAD",
  ] {
    assert_eq!(origin(source), MessageOrigin::Reused, "{source}");
  }
  assert_eq!(origin("git commit --fixup abc123"), MessageOrigin::Derived);
  assert_eq!(origin("git commit --squash=abc123"), MessageOrigin::Derived);
  assert_eq!(origin("git commit --amend --no-edit"), MessageOrigin::Kept);
  assert_eq!(origin("git commit --no-edit"), MessageOrigin::Written);
  let reworded = args("git commit --amend --no-edit -m 'fix(x): y'").unwrap();
  assert_eq!(reworded.origin, MessageOrigin::Written);
  assert!(reworded.amend);
}

#[test]
fn a_value_that_looks_like_an_option_is_still_a_value() {
  let found = args("git commit -m -x -Fmsg").unwrap();
  assert_eq!(found.messages, some(&["-x"]));
  assert_eq!(found.files, some(&["msg"]));
}
