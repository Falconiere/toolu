use toolu_shell::analyze;

use super::*;

#[test]
fn a_static_push_is_found_and_an_oversize_command_is_not() {
  assert!(is_git_push(&analyze(
    "git -C repo push origin HEAD:feature"
  )));
  assert!(!is_git_push(&analyze("echo git push")));
}
