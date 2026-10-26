use super::retagging;

fn retags(command: &str) -> bool {
  retagging(&toolu_shell::analyze(command)).is_some()
}

#[test]
fn only_a_tag_created_on_another_commit_is_a_retag() {
  for command in ["git tag -a v8.4.2 HEAD~3 -m x", "git tag v8.4.2 \"$C\""] {
    assert!(retags(command), "missed: {command}");
  }
  for command in [
    "git tag -a \"$TAG\" -m \"toolu $TAG\"",
    "git tag v8.4.2 HEAD",
    "git tag --contains HEAD~1 'v8*'",
    "git tag --points-at HEAD~1",
    "git tag -d v0.0.1",
  ] {
    assert!(!retags(command), "blocked: {command}");
  }
}
